use anyhow::{Context, Result, bail, ensure};
use chrono::{DateTime, FixedOffset, Timelike, Utc};
use fs2::FileExt;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    env,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

#[derive(Deserialize)]
struct Bairro {
    id: String,
    name: String,
    latitude: f64,
    longitude: f64,
}
#[derive(Deserialize)]
struct ApiResponse {
    current: Current,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum ApiRows {
    Multiple(Vec<ApiResponse>),
    Single(ApiResponse),
}
impl ApiRows {
    fn into_vec(self) -> Vec<ApiResponse> {
        match self {
            Self::Multiple(rows) => rows,
            Self::Single(row) => vec![row],
        }
    }
}
#[derive(Deserialize)]
struct Current {
    temperature_2m: f64,
    time: String,
}
#[derive(Serialize, Deserialize)]
struct Reading {
    bairro_id: String,
    bairro: String,
    latitude: f64,
    longitude: f64,
    temperatura_c: f64,
    horario_dado: String,
}
#[derive(Serialize, Deserialize)]
struct Snapshot {
    hora: String,
    coletado_em: String,
    fonte: String,
    leituras: Vec<Reading>,
}

fn now() -> DateTime<FixedOffset> {
    Utc::now().with_timezone(&FixedOffset::west_opt(3 * 3600).unwrap())
}
fn slot(time: DateTime<FixedOffset>) -> String {
    time.format("%Y-%m-%dT%H:00:00-03:00").to_string()
}
fn log_path(dir: &Path, time: DateTime<FixedOffset>) -> PathBuf {
    dir.join(format!("fortaleza-{}.jsonl", time.format("%Y-%m-%d")))
}
fn load(path: &Path) -> Result<Vec<Snapshot>> {
    let contents = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(error) => return Err(error).context("Não foi possível ler o log"),
    };
    contents
        .lines()
        .map(|line| {
            serde_json::from_str(line).context("Log inválido; arquivo preservado para inspeção")
        })
        .collect()
}
fn save(path: &Path, snapshot: Snapshot) -> Result<bool> {
    let mut rows = load(path)?;
    if rows.iter().any(|row| row.hora == snapshot.hora) {
        return Ok(false);
    }
    rows.push(snapshot);
    // Replace atomically so a crash cannot leave a partially appended JSON line.
    let temporary = path.with_extension("jsonl.tmp");
    let mut file = File::create(&temporary)?;
    for row in rows {
        serde_json::to_writer(&mut file, &row)?;
        file.write_all(b"\n")?;
    }
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    File::open(path.parent().context("Diretório de logs inválido")?)?.sync_all()?;
    Ok(true)
}
fn collect(client: &Client, bairros: &[Bairro], hour: DateTime<FixedOffset>) -> Result<Snapshot> {
    let mut readings = Vec::with_capacity(bairros.len());
    for batch in bairros.chunks(40) {
        let latitudes = batch
            .iter()
            .map(|b| format!("{:.6}", b.latitude))
            .collect::<Vec<_>>()
            .join(",");
        let longitudes = batch
            .iter()
            .map(|b| format!("{:.6}", b.longitude))
            .collect::<Vec<_>>()
            .join(",");
        let response = client
            .get("https://api.open-meteo.com/v1/forecast")
            .query(&[
                ("latitude", latitudes.as_str()),
                ("longitude", longitudes.as_str()),
                ("current", "temperature_2m"),
                ("timezone", "America/Fortaleza"),
                ("forecast_days", "1"),
            ])
            .send()?
            .error_for_status()?
            .json::<ApiRows>()?
            .into_vec();
        ensure!(
            response.len() == batch.len(),
            "API retornou quantidade incorreta de bairros"
        );
        for (bairro, row) in batch.iter().zip(response) {
            let timestamp =
                DateTime::parse_from_rfc3339(&format!("{}:00-03:00", row.current.time))?;
            let age = now().signed_duration_since(timestamp).num_seconds();
            ensure!(
                (-900..=7200).contains(&age),
                "API retornou horário fora do esperado"
            );
            ensure!(
                row.current.temperature_2m.is_finite()
                    && (-20.0..=65.0).contains(&row.current.temperature_2m),
                "Temperatura inválida"
            );
            readings.push(Reading {
                bairro_id: bairro.id.clone(),
                bairro: bairro.name.clone(),
                latitude: bairro.latitude,
                longitude: bairro.longitude,
                temperatura_c: row.current.temperature_2m,
                horario_dado: timestamp.to_rfc3339(),
            });
        }
    }
    Ok(Snapshot {
        hora: slot(hour),
        coletado_em: now().to_rfc3339(),
        fonte: "Open-Meteo; temperatura do ar a 2 m, estimada por modelo".into(),
        leituras: readings,
    })
}
fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help") {
        println!(
            "fortaleza-collector [--once] [--output DIRETORIO]\nSem --once: coleta ao iniciar e a cada hora cheia. Logs diários em JSONL (UTC-3)."
        );
        return Ok(());
    }
    let mut once = false;
    let mut directory = PathBuf::from("logs");
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--once" => once = true,
            "--output" => {
                i += 1;
                directory = args
                    .get(i)
                    .context("Informe um diretório após --output")?
                    .into();
            }
            value => bail!("Argumento desconhecido: {value}"),
        }
        i += 1;
    }
    fs::create_dir_all(&directory)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join(".collector.lock"))?;
    lock.try_lock_exclusive()
        .context("Outro coletor já está usando este diretório")?;
    let bairros: Vec<Bairro> = serde_json::from_str(include_str!("../bairros.json"))?;
    ensure!(
        bairros.len() == 121 && bairros.iter().map(|b| &b.id).collect::<HashSet<_>>().len() == 121,
        "Esperados 121 bairros únicos"
    );
    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .user_agent("fortaleza-collector/0.1.0")
        .build()?;
    println!(
        "{} Coletor iniciado: 121 bairros; logs em {}",
        now().to_rfc3339(),
        directory.display()
    );
    loop {
        let hour = now();
        let path = log_path(&directory, hour);
        if !load(&path)?.iter().any(|row| row.hora == slot(hour)) {
            let mut success = false;
            for attempt in 1..=3 {
                match collect(&client, &bairros, hour) {
                    Ok(snapshot) => {
                        save(&path, snapshot)?;
                        println!(
                            "{} Salvos 121 bairros em {}",
                            now().to_rfc3339(),
                            path.display()
                        );
                        success = true;
                        break;
                    }
                    Err(error) => {
                        eprintln!("{} Tentativa {attempt}/3: {error:#}", now().to_rfc3339());
                        if once {
                            return Err(error);
                        }
                        if attempt < 3 {
                            let remaining = 3600 - (now().timestamp().rem_euclid(3600));
                            if remaining <= 300 {
                                break;
                            }
                            thread::sleep(Duration::from_secs(300));
                            if slot(now()) != slot(hour) {
                                break;
                            }
                        }
                    }
                }
            }
            if !success {
                eprintln!(
                    "{} Hora sem coleta; nova tentativa na próxima hora cheia",
                    now().to_rfc3339()
                );
            }
        } else {
            println!(
                "{} Hora já coletada; aguardando próxima hora",
                now().to_rfc3339()
            );
        }
        if once {
            break;
        }
        let current = now();
        if slot(current) != slot(hour) {
            continue;
        }
        let seconds = 3600 - u64::from(current.minute()) * 60 - u64::from(current.second());
        thread::sleep(Duration::from_secs(seconds));
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{} ERRO: {error:#}", now().to_rfc3339());
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot() -> Snapshot {
        Snapshot {
            hora: "2026-11-09T15:00:00-03:00".into(),
            coletado_em: "2026-11-09T15:10:00-03:00".into(),
            fonte: "test".into(),
            leituras: vec![],
        }
    }
    #[test]
    fn handles_single_and_multiple_locations() {
        let row = r#"{"current":{"temperature_2m":30.2,"time":"2026-11-09T15:00"}}"#;
        assert_eq!(
            serde_json::from_str::<ApiRows>(row)
                .unwrap()
                .into_vec()
                .len(),
            1
        );
        assert_eq!(
            serde_json::from_str::<ApiRows>(&format!("[{row},{row}]"))
                .unwrap()
                .into_vec()
                .len(),
            2
        );
    }
    #[test]
    fn daily_rotation_uses_fortaleza_date() {
        let utc = DateTime::parse_from_rfc3339("2026-11-10T02:00:00Z").unwrap();
        let local = utc.with_timezone(&FixedOffset::west_opt(10800).unwrap());
        assert_eq!(
            log_path(Path::new("logs"), local),
            PathBuf::from("logs/fortaleza-2026-11-09.jsonl")
        );
        assert_eq!(slot(local), "2026-11-09T23:00:00-03:00");
    }
    #[test]
    fn saves_atomically_and_deduplicates_after_restart() {
        let dir = env::temp_dir().join(format!("fortaleza-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("day.jsonl");
        assert!(save(&path, snapshot()).unwrap());
        assert!(!save(&path, snapshot()).unwrap());
        assert_eq!(load(&path).unwrap().len(), 1);
        fs::write(&path, "damaged\n").unwrap();
        assert!(save(&path, snapshot()).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "damaged\n");
        fs::remove_dir_all(&dir).unwrap();
    }
}
