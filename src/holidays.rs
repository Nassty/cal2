use crate::{
    HM,
    config,
    error::{CalError, Result},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, BufWriter, Write},
    path::PathBuf,
};

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
    pub fn from_country(country: Option<String>) -> Result<Self> {
        let Some(country) = country else {
            return Ok(Provider::default());
        };

        let trimmed = country.trim();
        if trimmed.is_empty() {
            return Err(CalError::Config("--country cannot be empty".to_string()));
        }

        let upper = trimmed.to_uppercase();
        if !(2..=3).contains(&upper.len()) {
            return Err(CalError::Config(
                "--country must be a 2- or 3-letter ISO code".to_string(),
            ));
        }

        if !upper.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(CalError::Config(
                "--country must contain only ASCII letters".to_string(),
            ));
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

    fn fetch(&self, year: i32) -> Result<HM> {
        match self {
            Provider::ArgentinaDatos => fetch_argentina(year),
            Provider::OpenHolidays { country_code } => fetch_openholidays(year, country_code),
        }
    }
}

pub fn get_filename(year: i32, provider: &Provider) -> PathBuf {
    let basename = if provider.is_default() {
        format!("hm-{year}")
    } else {
        format!("hm-{}-{year}", provider.slug())
    };
    config::data_dir().join(basename)
}

type LegacyHM = HashMap<(u32, u32), bool>;
const MAX_CACHE_BYTES: u64 = 10 * 1024 * 1024;

pub fn load(path: &PathBuf) -> Result<Option<HM>> {
    let metadata = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };

    if metadata.len() > MAX_CACHE_BYTES {
        return Err(CalError::Cache(format!(
            "cache {} exceeds {MAX_CACHE_BYTES} bytes",
            path.display()
        )));
    }

    let bytes = fs::read(path)?;

    if let Ok(resp) = bincode::deserialize::<HM>(&bytes) {
        return Ok(Some(resp));
    }

    if let Ok(legacy) = bincode::deserialize::<LegacyHM>(&bytes) {
        let mut migrated = HashMap::new();
        for ((day, month), is_holiday) in legacy {
            if is_holiday {
                let name = format!("Legacy holiday ({day:02}/{month:02})");
                migrated.insert((day, month), HolidayEntry::custom(name));
            }
        }
        save(path, &migrated)?;
        return Ok(Some(migrated));
    }

    Err(CalError::Cache(format!(
        "failed to deserialize cache {}",
        path.display()
    )))
}

pub fn save(path: &PathBuf, hm: &HM) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    bincode::serialize_into(&mut writer, hm)?;
    writer.flush()?;
    Ok(())
}

pub fn get_holidays(year: i32, provider: &Provider) -> Result<HM> {
    let path = get_filename(year, provider);
    if let Some(hm) = load(&path)? {
        return Ok(hm);
    }

    let hm = provider.fetch(year)?;
    save(&path, &hm)?;
    Ok(hm)
}

fn fetch_argentina(year: i32) -> Result<HM> {
    let response =
        reqwest::blocking::get(format!("https://api.argentinadatos.com/v1/feriados/{year}"))?;
    let data = response.text()?;
    let entries: Vec<ArgentinaResp> = serde_json::from_str(&data)?;
    Ok(build_holidays(
        entries.into_iter().map(|resp| (resp.fecha, resp.nombre)),
    ))
}

fn fetch_openholidays(year: i32, country_code: &str) -> Result<HM> {
    let url = format!(
        "https://openholidaysapi.org/PublicHolidays?countryIsoCode={country_code}&languageIsoCode=EN&validFrom={year}-01-01&validTo={year}-12-31"
    );
    let response = reqwest::blocking::get(url)?;
    let data = response.text()?;
    let entries: Vec<OpenHolidayResp> = serde_json::from_str(&data)?;
    Ok(build_holidays(entries.into_iter().map(|resp| {
        let chosen = resp
            .name
            .iter()
            .find(|n| n.language.eq_ignore_ascii_case("EN"))
            .or_else(|| resp.name.first())
            .map(|n| n.text.clone())
            .unwrap_or_else(|| "Public holiday".to_string());
        (resp.start_date, chosen)
    })))
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
    let _year = parts.next()?;
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
        env,
        fs::{self, File},
        io::Write,
        time::SystemTime,
    };

    fn temp_path(label: &str) -> PathBuf {
        let mut path = env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        path.push(format!("cal2-{label}-{nanos}"));
        path
    }

    #[test]
    fn load_returns_none_for_missing_file() {
        let path = temp_path("missing");
        let result = load(&path).expect("load should not error for missing file");
        assert!(result.is_none());
    }

    #[test]
    fn save_and_load_roundtrip_preserves_holidays() {
        let mut hm = HashMap::new();
        hm.insert((1, 1), HolidayEntry::official("New Year's Day".to_string()));
        hm.insert(
            (25, 12),
            HolidayEntry::official("Christmas Day".to_string()),
        );

        let path = temp_path("roundtrip");
        save(&path, &hm).expect("save should succeed");
        let raw_bytes = fs::read(&path).expect("able to read serialized data");
        let raw_result: std::result::Result<HM, _> = bincode::deserialize(&raw_bytes);
        assert!(
            raw_result.is_ok(),
            "raw deserialize failed: {:?}",
            raw_result.err()
        );
        let loaded = load(&path)
            .expect("load should succeed after save")
            .expect("cache should exist after saving");

        assert_eq!(loaded, hm);
        fs::remove_file(&path).expect("able to remove temp cache");
    }

    #[test]
    fn load_migrates_legacy_boolean_cache() {
        let path = temp_path("legacy");
        let mut legacy_map = HashMap::new();
        legacy_map.insert((1, 1), true);
        legacy_map.insert((2, 1), false);

        {
            let mut file = File::create(&path).expect("create legacy file");
            bincode::serialize_into(&mut file, &legacy_map).expect("serialize legacy cache");
        }

        let raw_bytes = fs::read(&path).expect("read legacy cache");
        let legacy_raw: std::result::Result<LegacyHM, _> = bincode::deserialize(&raw_bytes);
        assert!(
            legacy_raw.is_ok(),
            "legacy raw deserialize failed: {:?}",
            legacy_raw.err()
        );

        let migrated = load(&path)
            .expect("legacy cache should migrate")
            .expect("migrated cache should exist");
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

        fs::remove_file(&path).expect("remove migrated cache");
    }

    #[test]
    fn get_filename_places_cache_under_data_directory_for_default_provider() {
        let year = 2030;
        let path = get_filename(year, &Provider::default());
        let path_str = path.to_string_lossy();
        assert!(
            path_str.ends_with(&format!("hm-{year}")),
            "unexpected cache filename: {path_str}"
        );
    }

    #[test]
    fn get_filename_includes_provider_slug_when_not_default() {
        let provider = Provider::OpenHolidays {
            country_code: "US".to_string(),
        };
        let year = 2030;
        let path = get_filename(year, &provider);
        let path_str = path.to_string_lossy();
        assert!(
            path_str.ends_with("hm-openholidays-us-2030"),
            "unexpected cache filename: {path_str}"
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
        let path = temp_path("too-big");
        let mut file = File::create(&path).expect("create temp file");
        let oversize = vec![0_u8; (10 * 1024 * 1024) + 1];
        file.write_all(&oversize).expect("write oversize cache");

        let result = load(&path);
        assert!(result.is_err(), "expected oversized cache to be rejected");

        fs::remove_file(&path).expect("remove oversize temp file");
    }

    #[test]
    #[serial]
    fn get_holidays_uses_cached_file_when_present() {
        let temp_dir = temp_path("home-holidays");
        fs::create_dir_all(&temp_dir).expect("create temp dir");

        let previous_data = env::var("XDG_DATA_HOME").ok();
        let previous_home = env::var("HOME").ok();
        unsafe {
            env::set_var("XDG_DATA_HOME", temp_dir.join("data"));
            env::set_var("HOME", &temp_dir);
        }

        let provider = Provider::default();
        let year = 2035;
        let path = get_filename(year, &provider);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create cache parent");
        }

        let mut hm = HashMap::new();
        hm.insert(
            (2, 1),
            HolidayEntry::official("Test cached holiday".to_string()),
        );
        save(&path, &hm).expect("save cached map");

        let loaded = get_holidays(year, &provider).expect("load cached holidays");
        assert_eq!(loaded, hm);

        unsafe {
            match previous_data {
                Some(v) => env::set_var("XDG_DATA_HOME", v),
                None => env::remove_var("XDG_DATA_HOME"),
            }
            match previous_home {
                Some(v) => env::set_var("HOME", v),
                None => env::remove_var("HOME"),
            }
        }
        let _ = fs::remove_dir_all(&temp_dir);
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
        let valid = hm.get(&(1, 5)).expect("expected valid date to be recorded");
        assert_eq!(valid.name, "Valid");
        assert!(hm.get(&(1, 13)).is_some());
        assert!(hm.iter().all(|(_, entry)| entry.name != "Bad"));
    }
}
