use anyhow::{Context, Result, bail, ensure};
use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    env,
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tiny_http::{Header, Response, Server, StatusCode};

static ASSETS: include_dir::Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/../dashboard-wasm/generated");

fn validate(row: &Value) -> Result<()> {
    DateTime::parse_from_rfc3339(row["hora"].as_str().context("Hora ausente")?)?;
    DateTime::parse_from_rfc3339(row["coletado_em"].as_str().context("Horário ausente")?)?;
    let rows = row["leituras"].as_array().context("Leituras ausentes")?;
    ensure!(rows.len() == 121, "Coleta incompleta");
    let mut ids = HashSet::new();
    for r in rows {
        ensure!(
            ids.insert(r["bairro_id"].as_str().context("ID ausente")?),
            "Bairro duplicado"
        );
        ensure!(
            r["bairro"].is_string() && r["temperatura_c"].as_f64().is_some_and(f64::is_finite),
            "Leitura inválida"
        );
        DateTime::parse_from_rfc3339(
            r["horario_dado"]
                .as_str()
                .context("Horário do dado ausente")?,
        )?;
    }
    Ok(())
}
fn log_date(name: &str) -> Option<NaiveDate> {
    let date = name.strip_prefix("fortaleza-")?.strip_suffix(".jsonl")?;
    let parsed = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    if parsed.to_string() == date {
        Some(parsed)
    } else {
        None
    }
}
fn read_logs(directory: &Path, now: DateTime<FixedOffset>) -> Result<Value> {
    let mut latest: Option<Value> = None;
    let mut latest_time = None;
    let mut recent = Vec::new();
    let mut issues = Vec::new();
    let entries = match fs::read_dir(directory) {
        Ok(e) => Some(e),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    if let Some(entries) = entries {
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(date) = log_date(&name) else {
                continue;
            };
            let result = (|| -> Result<Vec<Value>> {
                let mut rows = Vec::new();
                let mut hours = HashSet::new();
                for line in BufReader::new(File::open(entry.path())?).lines() {
                    let line = line?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    let row: Value = serde_json::from_str(&line)?;
                    validate(&row)?;
                    ensure!(
                        row["hora"].as_str().unwrap().starts_with(&date.to_string()),
                        "Data diferente do arquivo"
                    );
                    ensure!(
                        hours.insert(row["hora"].as_str().unwrap().to_string()),
                        "Hora duplicada"
                    );
                    rows.push(row);
                }
                Ok(rows)
            })();
            match result {
                Ok(rows) => {
                    for row in rows {
                        let collected =
                            DateTime::parse_from_rfc3339(row["coletado_em"].as_str().unwrap())?;
                        if collected > now {
                            continue;
                        }
                        if latest_time.is_none_or(|t| collected > t) {
                            latest_time = Some(collected);
                            latest = Some(row.clone());
                        }
                        let age = now.signed_duration_since(collected).num_milliseconds();
                        if (0..=86400000).contains(&age) {
                            recent.push(row);
                        }
                    }
                }
                Err(error) => {
                    eprintln!("{name}: {error:#}");
                    issues.push(format!("Não foi possível ler {name}."));
                }
            }
        }
    }
    recent.sort_by(|a, b| a["coletado_em"].as_str().cmp(&b["coletado_em"].as_str()));
    Ok(json!({"latest":latest,"recent":recent,"issues":issues,"serverTime":now.to_rfc3339()}))
}
fn service_status() -> Value {
    let mut child = match Command::new("systemctl")
        .args([
            "--user",
            "show",
            "fortaleza-collector.service",
            "--property=ActiveState",
            "--property=SubState",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return json!({"state":"unknown","detail":"unknown"}),
    };
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return json!({"state":"unknown","detail":"unknown"});
            }
        }
    }
    match child.wait_with_output() {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let value = |key: &str| {
                text.lines()
                    .find_map(|line| line.strip_prefix(key))
                    .unwrap_or("unknown")
                    .to_string()
            };
            json!({"state":value("ActiveState="),"detail":value("SubState=")})
        }
        _ => json!({"state":"unknown","detail":"unknown"}),
    }
}
fn response(status: u16, bytes: Vec<u8>, mime: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_data(bytes)
        .with_status_code(StatusCode(status))
        .with_header(Header::from_bytes("Content-Type", mime).unwrap())
        .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
        .with_header(Header::from_bytes("X-Content-Type-Options", "nosniff").unwrap())
}
fn run() -> Result<()> {
    let mut logs = PathBuf::from("logs");
    let mut port = 3040u16;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--logs" => logs = args.next().context("Informe --logs DIRETORIO")?.into(),
            "--port" => {
                port = args.next().context("Informe --port NUMERO")?.parse()?;
                ensure!(port > 0, "Porta inválida");
            }
            "--help" => {
                println!("fortaleza-dashboard [--logs DIRETORIO] [--port 3040]");
                return Ok(());
            }
            _ => bail!("Argumento desconhecido: {arg}"),
        }
    }
    let server = Server::http(("127.0.0.1", port)).map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("Painel Rust + WASM: http://localhost:{port}/painel");
    for request in server.incoming_requests() {
        let url = request.url().split('?').next().unwrap_or("");
        let answer = if request.method() != &tiny_http::Method::Get {
            response(
                405,
                b"Metodo nao permitido".to_vec(),
                "text/plain; charset=utf-8",
            )
        } else {
            match url {
                "/" | "/painel" | "/painel/" => response(
                    200,
                    include_bytes!("../../../dashboard-wasm/static/index.html").to_vec(),
                    "text/html; charset=utf-8",
                ),
                "/bootstrap.js" => response(
                    200,
                    include_bytes!("../../../dashboard-wasm/static/bootstrap.js").to_vec(),
                    "text/javascript; charset=utf-8",
                ),
                path if path.starts_with("/assets/") => {
                    let name = &path[8..];
                    if let Some(file) = ASSETS.get_file(name) {
                        let mime = if name.ends_with(".wasm") {
                            "application/wasm"
                        } else {
                            "text/javascript; charset=utf-8"
                        };
                        response(200, file.contents().to_vec(), mime)
                    } else {
                        response(404, b"Nao encontrado".to_vec(), "text/plain")
                    }
                }
                "/api/collector" => {
                    let now = Utc::now().with_timezone(&FixedOffset::west_opt(10800).unwrap());
                    match read_logs(&logs, now) {
                        Ok(mut data) => {
                            data["service"] = service_status();
                            response(
                                200,
                                serde_json::to_vec(&data)?,
                                "application/json; charset=utf-8",
                            )
                        }
                        Err(e) => {
                            eprintln!("{e:#}");
                            response(
                                500,
                                br#"{"error":"Nao foi possivel ler os logs"}"#.to_vec(),
                                "application/json",
                            )
                        }
                    }
                }
                _ => response(404, b"Nao encontrado".to_vec(), "text/plain; charset=utf-8"),
            }
        };
        if let Err(e) = request.respond(answer) {
            eprintln!("Resposta interrompida: {e}");
        }
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn row(hour: &str, collected: &str) -> Value {
        json!({"hora":hour,"coletado_em":collected,"leituras":(0..121).map(|i|json!({"bairro_id":i.to_string(),"bairro":format!("Bairro {i}"),"temperatura_c":28.2,"horario_dado":collected})).collect::<Vec<_>>()})
    }
    #[test]
    fn date_and_validation() {
        assert!(log_date("fortaleza-2026-02-31.jsonl").is_none());
        assert!(log_date("../2026-11-09").is_none());
        assert!(validate(&json!({"leituras":[]})).is_err());
    }
    #[test]
    fn rolling_day_and_corrupt_files() {
        let directory =
            env::temp_dir().join(format!("fortaleza-rust-panel-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let previous = row("2026-11-08T23:00:00-03:00", "2026-11-08T23:50:00-03:00");
        let old = row("2026-11-08T22:00:00-03:00", "2026-11-08T22:00:00-03:00");
        let current = row("2026-11-09T18:00:00-03:00", "2026-11-09T18:16:00-03:00");
        fs::write(
            directory.join("fortaleza-2026-11-08.jsonl"),
            format!("{old}\n{previous}\n"),
        )
        .unwrap();
        fs::write(
            directory.join("fortaleza-2026-11-09.jsonl"),
            format!("{current}\n"),
        )
        .unwrap();
        let now = DateTime::parse_from_rfc3339("2026-11-09T23:30:00-03:00").unwrap();
        let data = read_logs(&directory, now).unwrap();
        assert_eq!(data["recent"].as_array().unwrap().len(), 2);
        assert_eq!(data["latest"]["coletado_em"], current["coletado_em"]);
        fs::write(directory.join("fortaleza-2026-11-09.jsonl"), "corrupt\n").unwrap();
        let data = read_logs(&directory, now).unwrap();
        assert_eq!(data["issues"].as_array().unwrap().len(), 1);
        assert_eq!(data["recent"].as_array().unwrap().len(), 1);
        fs::remove_dir_all(directory).unwrap();
    }
}
