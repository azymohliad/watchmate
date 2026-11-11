//! Open-Meteo -> InfiniTime SimpleWeatherService v0.
use anyhow::{Context, Result};
use chrono::{Local, Offset};
use infinitime::bt::InfiniTime;
use serde::Deserialize;

#[derive(Deserialize)]
struct Response { current: Current, daily: Daily }
#[derive(Deserialize)]
struct Current { temperature_2m: f64, weather_code: u16 }
#[derive(Deserialize)]
struct Daily { temperature_2m_min: Vec<f64>, temperature_2m_max: Vec<f64>, weather_code: Vec<u16> }

fn icon(code: u16) -> u8 {
    match code {
        0 => 0, 1 | 2 => 1, 3 => 3, 45 | 48 => 8,
        51..=57 | 80..=82 => 4, 61..=67 => 5,
        71..=77 | 85 | 86 => 7, 95..=99 => 6, _ => 2,
    }
}
fn temperature(value: f64) -> i16 {
    (value * 100.0).round().clamp(i16::MIN as f64, i16::MAX as f64) as i16
}
fn timestamp() -> [u8; 8] {
    // InfiniTime expects a local-wall-clock Unix timestamp, not UTC.
    let now = Local::now();
    (now.timestamp() + i64::from(now.offset().fix().local_minus_utc())).to_le_bytes()
}
fn encode_current(data: &Response, name: &str) -> [u8; 49] {
    let mut buf = [0u8; 49];
    buf[2..10].copy_from_slice(&timestamp());
    for (i, t) in [data.current.temperature_2m, data.daily.temperature_2m_min[0], data.daily.temperature_2m_max[0]].iter().enumerate() {
        buf[10 + i * 2..12 + i * 2].copy_from_slice(&temperature(*t).to_le_bytes());
    }
    let mut end = 0;
    for ch in name.chars() {
        let mut tmp = [0; 4];
        let bytes = ch.encode_utf8(&mut tmp).as_bytes();
        if end + bytes.len() > 32 { break; }
        buf[16 + end..16 + end + bytes.len()].copy_from_slice(bytes);
        end += bytes.len();
    }
    buf[48] = icon(data.current.weather_code);
    buf
}
fn encode_forecast(data: &Response) -> [u8; 36] {
    let mut buf = [0u8; 36];
    buf[0] = 1;
    buf[2..10].copy_from_slice(&timestamp());
    let days = 5.min(data.daily.temperature_2m_min.len()).min(data.daily.temperature_2m_max.len()).min(data.daily.weather_code.len());
    buf[10] = days as u8;
    for i in 0..days {
        let offset = 11 + 5 * i;
        buf[offset..offset + 2].copy_from_slice(&temperature(data.daily.temperature_2m_min[i]).to_le_bytes());
        buf[offset + 2..offset + 4].copy_from_slice(&temperature(data.daily.temperature_2m_max[i]).to_le_bytes());
        buf[offset + 4] = icon(data.daily.weather_code[i]);
    }
    buf
}

#[derive(Deserialize)]
struct GeocodingResponse { #[serde(default)] results: Vec<GeocodingResult> }

#[derive(Deserialize, Clone, Debug)]
pub struct GeocodingResult {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    #[serde(default)]
    pub admin1: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
}

impl GeocodingResult {
    pub fn label(&self) -> String {
        [Some(self.name.as_str()), self.admin1.as_deref(), self.country.as_deref()]
            .into_iter().flatten().collect::<Vec<_>>().join(", ")
    }
}

pub async fn search_city(query: &str) -> Result<Vec<GeocodingResult>> {
    let data: GeocodingResponse = reqwest::Client::new().get("https://geocoding-api.open-meteo.com/v1/search")
        .query(&[("name", query), ("count", "10"), ("language", "de"), ("format", "json")])
        .timeout(std::time::Duration::from_secs(15)).send().await?
        .error_for_status()?.json().await?;
    Ok(data.results)
}

pub async fn update(device: &InfiniTime, latitude: f64, longitude: f64, name: &str) -> Result<()> {
    anyhow::ensure!((-90.0..=90.0).contains(&latitude) && (-180.0..=180.0).contains(&longitude), "Invalid weather coordinates");
    let data: Response = reqwest::Client::new().get("https://api.open-meteo.com/v1/forecast")
        .query(&[("latitude", latitude.to_string()), ("longitude", longitude.to_string()),
            ("current", "temperature_2m,weather_code".into()),
            ("daily", "temperature_2m_min,temperature_2m_max,weather_code".into()),
            ("forecast_days", "5".into()), ("timezone", "auto".into())])
        .timeout(std::time::Duration::from_secs(15)).send().await?
        .error_for_status()?.json().await?;
    anyhow::ensure!(!data.daily.temperature_2m_min.is_empty() && !data.daily.temperature_2m_max.is_empty(), "Missing daily weather data");
    device.write_weather(&encode_current(&data, name)).await.context("Sending current weather")?;
    device.write_weather(&encode_forecast(&data)).await.context("Sending forecast")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packet_layout() {
        let data = Response { current: Current { temperature_2m: -1.5, weather_code: 95 }, daily: Daily {
            temperature_2m_min: vec![-4.0; 5], temperature_2m_max: vec![2.0; 5], weather_code: vec![3; 5] } };
        let current = encode_current(&data, "Test");
        assert_eq!(current.len(), 49);
        assert_eq!(i16::from_le_bytes([current[10], current[11]]), -150);
        assert_eq!(current[48], 6);
        let forecast = encode_forecast(&data);
        assert_eq!(forecast.len(), 36);
        assert_eq!(forecast[10], 5);
        assert_eq!(i16::from_le_bytes([forecast[11], forecast[12]]), -400);
    }
}
