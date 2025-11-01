use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufWriter, Write},
};

use crate::HM;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HolidayKind {
    Official,
    Custom,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HolidayEntry {
    pub name: String,
    pub kind: HolidayKind,
}

impl HolidayEntry {
    pub fn official(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: HolidayKind::Official,
        }
    }

    pub fn custom(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: HolidayKind::Custom,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ArgentinaResp {
    fecha: String,
    nombre: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenHolidayResp {
    #[serde(rename = "startDate")]
    start_date: String,
    name: Vec<OpenHolidayName>,
}

#[derive(Debug, Deserialize)]
struct OpenHolidayName {
    language: String,
    text: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum Provider {
    #[default]
    ArgentinaDatos,
    OpenHolidays {
        country_code: String,
    },
}

impl Provider {
    pub fn from_country(country: Option<String>) -> Result<Self, String> {
        let Some(country) = country else {
            return Ok(Provider::default());
        };

        let trimmed = country.trim();
        if trimmed.is_empty() {
            return Err("--country cannot be empty".to_string());
        }

        let upper = trimmed.to_uppercase();
        if !(2..=3).contains(&upper.len()) {
            return Err("--country must be a 2- or 3-letter ISO code".to_string());
        }

        if !upper.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err("--country must contain only ASCII letters".to_string());
        }

        if upper == "AR" {
            Ok(Provider::ArgentinaDatos)
        } else {
            Ok(Provider::OpenHolidays {
                country_code: upper,
            })
        }
    }

    fn is_default(&self) -> bool {
        matches!(self, Provider::ArgentinaDatos)
    }

    fn slug(&self) -> String {
        match self {
            Provider::ArgentinaDatos => "argentina-datos".to_string(),
            Provider::OpenHolidays { country_code } => {
                format!("openholidays-{}", country_code.to_lowercase())
            }
        }
    }

    fn fetch(&self, year: i32) -> HM {
        match self {
            Provider::ArgentinaDatos => fetch_argentina(year),
            Provider::OpenHolidays { country_code } => fetch_openholidays(year, country_code),
        }
    }
}

pub fn get_filename(year: i32, provider: &Provider) -> String {
    let basename = if provider.is_default() {
        format!("hm-{year}")
    } else {
        format!("hm-{}-{year}", provider.slug())
    };
    shellexpand::tilde(&format!("~/.config/{basename}")).to_string()
}

type LegacyHM = HashMap<(u32, u32), bool>;

pub fn load(fname: &str) -> Result<HM, ()> {
    let meta = fs::metadata(fname).map_err(|_| ())?;
    const MAX_CACHE_BYTES: u64 = 10 * 1024 * 1024;
    if meta.len() > MAX_CACHE_BYTES {
        return Err(());
    }

    let bytes = fs::read(fname).map_err(|_| ())?;

    if let Ok(resp) = bincode::deserialize::<HM>(&bytes) {
        return Ok(resp);
    }

    if let Ok(legacy) = bincode::deserialize::<LegacyHM>(&bytes) {
        let mut migrated = HashMap::new();
        for ((day, month), is_holiday) in legacy {
            if is_holiday {
                let name = format!("Legacy holiday ({day:02}/{month:02})");
                migrated.insert((day, month), HolidayEntry::custom(name));
            }
        }
        save(fname, &migrated);
        return Ok(migrated);
    }

    Err(())
}

pub fn save(fname: &str, hm: &HM) {
    let file = File::create(fname).unwrap();
    let mut writer = BufWriter::new(file);
    bincode::serialize_into(&mut writer, hm).unwrap();
    writer.flush().unwrap();
}

pub fn get_holidays(year: i32, provider: &Provider) -> HM {
    let fname = get_filename(year, provider);
    if let Ok(hm) = load(&fname) {
        return hm;
    }
    let hm = provider.fetch(year);
    save(&fname, &hm);
    hm
}

fn fetch_argentina(year: i32) -> HM {
    let data = reqwest::blocking::get(format!("https://api.argentinadatos.com/v1/feriados/{year}"))
        .unwrap()
        .text()
        .unwrap();
    let v: Vec<ArgentinaResp> = serde_json::from_str(&data).unwrap();
    build_holidays(v.into_iter().map(|resp| (resp.fecha, resp.nombre)))
}

fn fetch_openholidays(year: i32, country_code: &str) -> HM {
    let url = format!(
        "https://openholidaysapi.org/PublicHolidays?countryIsoCode={country_code}&languageIsoCode=EN&validFrom={year}-01-01&validTo={year}-12-31"
    );
    let data = reqwest::blocking::get(url).unwrap().text().unwrap();
    let v: Vec<OpenHolidayResp> = serde_json::from_str(&data).unwrap();
    build_holidays(v.into_iter().map(|resp| {
        let OpenHolidayResp {
            start_date,
            name: names,
        } = resp;
        let chosen = names
            .iter()
            .find(|n| n.language.eq_ignore_ascii_case("EN"))
            .or_else(|| names.first())
            .map(|n| n.text.clone())
            .unwrap_or_else(|| "Public holiday".to_string());
        (start_date, chosen)
    }))
}

fn build_holidays<I>(entries: I) -> HM
where
    I: IntoIterator<Item = (String, String)>,
{
    let mut hm = HashMap::new();
    for (date, name) in entries {
        if let Some((day, month)) = parse_date(&date) {
            hm.insert((day, month), HolidayEntry::official(name));
        }
    }
    hm
}

fn parse_date(date: &str) -> Option<(u32, u32)> {
    let mut parts = date.splitn(3, '-');
    parts.next()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    Some((day, month))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::{
        collections::HashMap,
        fs::{self, File},
        io::Write,
        path::Path,
        time::SystemTime,
    };

    fn temp_file(label: &str) -> String {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("cal2-{label}-{nanos}"));
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn load_returns_err_for_missing_file() {
        let fname = temp_file("missing");
        assert!(load(&fname).is_err());
    }

    #[test]
    fn save_and_load_roundtrip_preserves_holidays() {
        let mut hm = HashMap::new();
        hm.insert((1, 1), HolidayEntry::official("New Year's Day".to_string()));
        hm.insert(
            (25, 12),
            HolidayEntry::official("Christmas Day".to_string()),
        );

        let fname = temp_file("roundtrip");
        save(&fname, &hm);
        let raw_bytes = fs::read(&fname).unwrap();
        let raw_result: Result<HM, _> = bincode::deserialize(&raw_bytes);
        assert!(
            raw_result.is_ok(),
            "raw deserialize failed: {:?}",
            raw_result.err()
        );
        let loaded = load(&fname).expect("load should succeed after save");

        assert_eq!(loaded, hm);
        fs::remove_file(&fname).unwrap();
    }

    #[test]
    fn load_migrates_legacy_boolean_cache() {
        let legacy_fname = temp_file("legacy");
        let mut legacy_map = HashMap::new();
        legacy_map.insert((1, 1), true);
        legacy_map.insert((2, 1), false);

        {
            let mut file = File::create(&legacy_fname).unwrap();
            bincode::serialize_into(&mut file, &legacy_map).unwrap();
        }

        let raw_bytes = fs::read(&legacy_fname).unwrap();
        let legacy_raw: Result<LegacyHM, _> = bincode::deserialize(&raw_bytes);
        assert!(
            legacy_raw.is_ok(),
            "legacy raw deserialize failed: {:?}",
            legacy_raw.err()
        );

        let migrated = load(&legacy_fname).expect("legacy cache should migrate");
        assert_eq!(migrated.len(), 1);
        let entry = migrated
            .get(&(1, 1))
            .expect("holiday should be present after migration");
        assert_eq!(entry.kind, HolidayKind::Custom);
        assert!(
            entry.name.contains("Legacy"),
            "unexpected migrated name: {}",
            entry.name
        );

        fs::remove_file(&legacy_fname).unwrap();
    }

    #[test]
    fn get_filename_places_cache_under_config_directory_for_default_provider() {
        let year = 2030;
        let fname = get_filename(year, &Provider::default());
        assert!(
            fname.ends_with(&format!("hm-{year}")),
            "unexpected cache filename: {fname}"
        );
    }

    #[test]
    fn get_filename_includes_provider_slug_when_not_default() {
        let provider = Provider::OpenHolidays {
            country_code: "US".to_string(),
        };
        let year = 2030;
        let fname = get_filename(year, &provider);
        assert!(
            fname.ends_with("hm-openholidays-us-2030"),
            "unexpected cache filename: {fname}"
        );
    }

    #[test]
    fn provider_from_country_rejects_invalid_codes() {
        for invalid in ["", " ", "1", "U1", "UNIT", "U_S"] {
            assert!(
                Provider::from_country(Some(invalid.to_string())).is_err(),
                "expected error for invalid country: {invalid:?}"
            );
        }
    }

    #[test]
    fn provider_from_country_accepts_valid_iso_codes() {
        let provider =
            Provider::from_country(Some("us".to_string())).expect("valid country should work");
        assert_eq!(
            provider,
            Provider::OpenHolidays {
                country_code: "US".to_string()
            }
        );
    }

    #[test]
    fn provider_from_country_uses_argentina_for_ar() {
        let provider =
            Provider::from_country(Some("ar".to_string())).expect("AR should be accepted");
        assert_eq!(provider, Provider::ArgentinaDatos);
    }

    #[test]
    fn load_rejects_cache_larger_than_limit() {
        let fname = temp_file("too-big");
        let mut file = File::create(&fname).unwrap();
        let oversize = vec![0_u8; (10 * 1024 * 1024) + 1];
        file.write_all(&oversize).unwrap();

        assert!(
            load(&fname).is_err(),
            "expected oversized cache to be rejected"
        );

        fs::remove_file(&fname).unwrap();
    }

    #[test]
    #[serial]
    fn get_holidays_uses_cached_file_when_present() {
        let mut home_dir = std::env::temp_dir();
        home_dir.push(format!(
            "cal2-home-{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&home_dir).unwrap();
        let previous_home = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", &home_dir) };

        let provider = Provider::default();
        let year = 2035;
        let fname = get_filename(year, &provider);
        let path = Path::new(&fname);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }

        let mut hm = HashMap::new();
        hm.insert(
            (2, 1),
            HolidayEntry::official("Test cached holiday".to_string()),
        );
        save(&fname, &hm);

        let loaded = get_holidays(year, &provider);
        assert_eq!(loaded, hm);

        fs::remove_file(&fname).unwrap();
        unsafe {
            if let Some(prev) = previous_home {
                std::env::set_var("HOME", prev);
            } else {
                std::env::remove_var("HOME");
            }
        }
    }

    #[test]
    fn provider_slug_and_default_behavior() {
        let argentina = Provider::default();
        assert!(argentina.is_default());
        assert_eq!(argentina.slug(), "argentina-datos");

        let open = Provider::OpenHolidays {
            country_code: "CA".to_string(),
        };
        assert!(!open.is_default());
        assert_eq!(open.slug(), "openholidays-ca");
    }

    #[test]
    fn build_holidays_filters_invalid_dates() {
        let entries = vec![
            ("2024-05-01".to_string(), "Valid".to_string()),
            ("2024-13-01".to_string(), "Invalid".to_string()),
            ("not-a-date".to_string(), "Bad".to_string()),
        ];

        let hm = build_holidays(entries);
        assert_eq!(hm.get(&(1, 5)).unwrap().name, "Valid");
        assert!(
            hm.get(&(1, 13)).is_some(),
            "out-of-range month still stored"
        );
        assert!(
            hm.iter().all(|(_, entry)| entry.name != "Bad"),
            "malformed date should be ignored"
        );
    }
}
