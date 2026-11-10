use chrono::{DateTime, FixedOffset};
use serde::Deserialize;
#[derive(Deserialize, Debug)]
pub struct Region {
    pub id: String,
    pub name: String,
    pub path: String,
}
#[derive(Deserialize, Debug)]
pub struct Reading {
    pub bairro_id: String,
    pub bairro: String,
    pub temperatura_c: f64,
}
#[derive(Deserialize, Debug)]
pub struct Snapshot {
    pub coletado_em: String,
    pub leituras: Vec<Reading>,
}
#[derive(Deserialize, Debug)]
pub struct Service {
    pub state: String,
    pub detail: String,
}
#[derive(Deserialize, Debug)]
pub struct Data {
    pub latest: Option<Snapshot>,
    pub recent: Vec<Snapshot>,
    pub issues: Vec<String>,
    pub service: Service,
    #[serde(rename = "serverTime")]
    pub server_time: String,
}
pub fn time(s: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(s).ok()
}
pub fn clock(s: &str) -> String {
    time(s)
        .map(|t| {
            t.with_timezone(&FixedOffset::west_opt(10800).unwrap())
                .format("%H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "—".into())
}
pub fn stamp(ms: i64) -> String {
    DateTime::from_timestamp_millis(ms)
        .map(|t| clock(&t.to_rfc3339()))
        .unwrap_or_else(|| "—".into())
}
pub fn number(n: f64) -> String {
    format!("{n:.1}").replace('.', ",")
}
pub fn thermal(value: f64, low: f64, high: f64) -> [u8; 3] {
    let t = if low == high {
        0.5
    } else {
        ((value - low) / (high - low)).clamp(0.0, 1.0)
    };
    let stops = [
        [112., 157., 170.],
        [157., 178., 140.],
        [214., 181., 136.],
        [202., 125., 102.],
    ];
    let k = ((t * 3.).floor() as usize).min(2);
    let mix = t * 3. - k as f64;
    std::array::from_fn(|i| (stops[k][i] + (stops[k + 1][i] - stops[k][i]) * mix).round() as u8)
}
pub fn rings(path: &str) -> Vec<Vec<[f32; 2]>> {
    let mut result = Vec::new();
    let mut ring = Vec::new();
    let mut start = 0;
    let mut command = 'M';
    for (i, c) in path
        .char_indices()
        .chain(std::iter::once((path.len(), 'Z')))
    {
        if matches!(c, 'M' | 'L' | 'Z') {
            if start < i && matches!(command, 'M' | 'L') {
                let coordinates: Vec<_> = path[start..i]
                    .split(',')
                    .filter_map(|n| n.parse::<f32>().ok())
                    .collect();
                if coordinates.len() == 2 {
                    ring.push([coordinates[0], coordinates[1]]);
                }
            }
            if c == 'M' && !ring.is_empty() {
                result.push(std::mem::take(&mut ring));
            }
            if c == 'Z' && !ring.is_empty() {
                result.push(std::mem::take(&mut ring));
            }
            command = c;
            start = i + 1;
        }
    }
    result
}
pub fn contains(p: [f32; 2], rings: &[Vec<[f32; 2]>]) -> bool {
    let mut inside = false;
    for ring in rings {
        if ring.len() < 3 {
            continue;
        }
        let mut j = ring.len() - 1;
        for i in 0..ring.len() {
            let a = ring[i];
            let b = ring[j];
            if (a[1] > p[1]) != (b[1] > p[1])
                && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
            {
                inside = !inside;
            }
            j = i;
        }
    }
    inside
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn map_and_palette() {
        let map: Vec<Region> = serde_json::from_str(include_str!("../../data/map.json")).unwrap();
        assert_eq!(map.len(), 121);
        for r in map {
            assert!(!rings(&r.path).is_empty());
        }
        assert_eq!(thermal(24., 24., 34.), [112, 157, 170]);
        assert_eq!(thermal(34., 24., 34.), [202, 125, 102]);
        assert_eq!(clock("2026-11-10T02:00:00Z"), "23:00");
    }
    #[test]
    fn polygon_holes() {
        let rings = rings("M0,0L10,0L10,10L0,10ZM3,3L7,3L7,7L3,7Z");
        assert!(contains([1., 1.], &rings));
        assert!(!contains([5., 5.], &rings));
        assert!(!contains([11., 5.], &rings));
    }
}
