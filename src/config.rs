use crate::error::{CalError, Result};
use serde::{Deserialize, Serialize};
use std::{
    env,
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::PathBuf,
};

const APP_NAME: &str = "cal2";
const CONFIG_FILE: &str = "config.json";

/// Application configuration stored in XDG config directory
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_country: Option<String>,
}

/// Get the XDG config directory for cal2
/// Falls back to ~/.config/cal2 if XDG_CONFIG_HOME is not set
pub fn config_dir() -> PathBuf {
    let base = env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".config")
        });
    base.join(APP_NAME)
}

/// Get the XDG data directory for cal2
/// Falls back to ~/.local/share/cal2 if XDG_DATA_HOME is not set
pub fn data_dir() -> PathBuf {
    let base = env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".local").join("share")
        });
    base.join(APP_NAME)
}

/// Get the path to the config file
pub fn config_file_path() -> PathBuf {
    config_dir().join(CONFIG_FILE)
}

/// Load configuration from the config file
/// Returns default config if file doesn't exist
pub fn load_config() -> Result<Config> {
    let path = config_file_path();
    if !path.exists() {
        return Ok(Config::default());
    }

    let file = File::open(&path)?;
    let reader = BufReader::new(file);
    let config: Config = serde_json::from_reader(reader).map_err(|e| {
        CalError::Config(format!("failed to parse config file {}: {}", path.display(), e))
    })?;
    Ok(config)
}

/// Save configuration to the config file
pub fn save_config(config: &Config) -> Result<()> {
    let dir = config_dir();
    fs::create_dir_all(&dir)?;

    let path = config_file_path();
    let file = File::create(&path)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, config)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::time::SystemTime;

    struct TempEnv {
        previous_config: Option<String>,
        previous_data: Option<String>,
        previous_home: Option<String>,
        temp_dir: PathBuf,
    }

    impl TempEnv {
        fn new(label: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos();
            let temp_dir = env::temp_dir().join(format!("cal2-test-{label}-{nanos}"));
            fs::create_dir_all(&temp_dir).expect("create temp dir");

            let previous_config = env::var("XDG_CONFIG_HOME").ok();
            let previous_data = env::var("XDG_DATA_HOME").ok();
            let previous_home = env::var("HOME").ok();

            unsafe {
                env::set_var("XDG_CONFIG_HOME", temp_dir.join("config"));
                env::set_var("XDG_DATA_HOME", temp_dir.join("data"));
                env::set_var("HOME", &temp_dir);
            }

            Self {
                previous_config,
                previous_data,
                previous_home,
                temp_dir,
            }
        }
    }

    impl Drop for TempEnv {
        fn drop(&mut self) {
            unsafe {
                match &self.previous_config {
                    Some(v) => env::set_var("XDG_CONFIG_HOME", v),
                    None => env::remove_var("XDG_CONFIG_HOME"),
                }
                match &self.previous_data {
                    Some(v) => env::set_var("XDG_DATA_HOME", v),
                    None => env::remove_var("XDG_DATA_HOME"),
                }
                match &self.previous_home {
                    Some(v) => env::set_var("HOME", v),
                    None => env::remove_var("HOME"),
                }
            }
            let _ = fs::remove_dir_all(&self.temp_dir);
        }
    }

    #[test]
    #[serial]
    fn config_dir_uses_xdg_config_home() {
        let _env = TempEnv::new("config-dir");
        let dir = config_dir();
        assert!(dir.to_string_lossy().contains("config"));
        assert!(dir.to_string_lossy().ends_with("cal2"));
    }

    #[test]
    #[serial]
    fn data_dir_uses_xdg_data_home() {
        let _env = TempEnv::new("data-dir");
        let dir = data_dir();
        assert!(dir.to_string_lossy().contains("data"));
        assert!(dir.to_string_lossy().ends_with("cal2"));
    }

    #[test]
    #[serial]
    fn load_config_returns_default_when_no_file() {
        let _env = TempEnv::new("no-config");
        let config = load_config().expect("load should succeed");
        assert!(config.default_country.is_none());
    }

    #[test]
    #[serial]
    fn save_and_load_config_roundtrip() {
        let _env = TempEnv::new("config-roundtrip");
        let config = Config {
            default_country: Some("US".to_string()),
        };
        save_config(&config).expect("save should succeed");

        let loaded = load_config().expect("load should succeed");
        assert_eq!(loaded.default_country, Some("US".to_string()));
    }

    #[test]
    fn config_dir_falls_back_to_home() {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let temp_dir = env::temp_dir().join(format!("cal2-fallback-{nanos}"));
        fs::create_dir_all(&temp_dir).expect("create temp");

        let prev_config = env::var("XDG_CONFIG_HOME").ok();
        let prev_home = env::var("HOME").ok();

        unsafe {
            env::remove_var("XDG_CONFIG_HOME");
            env::set_var("HOME", &temp_dir);
        }

        let dir = config_dir();
        assert!(dir.to_string_lossy().contains(".config"));
        assert!(dir.to_string_lossy().ends_with("cal2"));

        unsafe {
            match prev_config {
                Some(v) => env::set_var("XDG_CONFIG_HOME", v),
                None => env::remove_var("XDG_CONFIG_HOME"),
            }
            match prev_home {
                Some(v) => env::set_var("HOME", v),
                None => env::remove_var("HOME"),
            }
        }
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn data_dir_falls_back_to_home() {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let temp_dir = env::temp_dir().join(format!("cal2-data-fallback-{nanos}"));
        fs::create_dir_all(&temp_dir).expect("create temp");

        let prev_data = env::var("XDG_DATA_HOME").ok();
        let prev_home = env::var("HOME").ok();

        unsafe {
            env::remove_var("XDG_DATA_HOME");
            env::set_var("HOME", &temp_dir);
        }

        let dir = data_dir();
        assert!(dir.to_string_lossy().contains(".local/share"));
        assert!(dir.to_string_lossy().ends_with("cal2"));

        unsafe {
            match prev_data {
                Some(v) => env::set_var("XDG_DATA_HOME", v),
                None => env::remove_var("XDG_DATA_HOME"),
            }
            match prev_home {
                Some(v) => env::set_var("HOME", v),
                None => env::remove_var("HOME"),
            }
        }
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
