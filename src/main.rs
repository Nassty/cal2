use clap::Parser;
use std::{collections::HashMap, ffi::OsString, process};

mod cli;
mod config;
mod display_month;
mod error;
mod holidays;

use error::Result;
use holidays::HolidayEntry;

type HM = HashMap<(u32, u32), HolidayEntry>;

pub fn run_with_args<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let args = cli::Args::parse_from(args);
    args.invoke()
}

fn main() {
    if let Err(err) = run_with_args(std::env::args()) {
        eprintln!("{err}");
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::holidays::{HolidayEntry, Provider, get_filename, save};
    use chrono::{Datelike, Utc};
    use serial_test::serial;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::SystemTime;

    #[test]
    #[serial]
    fn run_with_args_uses_cached_display() {
        let temp_dir: PathBuf = {
            let mut path = env::temp_dir();
            let nanos = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos();
            path.push(format!("cal2-home-main-run-{nanos}"));
            fs::create_dir_all(&path).expect("create temp dir");
            path
        };

        let previous_home = env::var("HOME").ok();
        let previous_data = env::var("XDG_DATA_HOME").ok();
        let previous_config = env::var("XDG_CONFIG_HOME").ok();
        unsafe {
            env::set_var("HOME", &temp_dir);
            env::set_var("XDG_DATA_HOME", temp_dir.join("data"));
            env::set_var("XDG_CONFIG_HOME", temp_dir.join("config"));
        }

        let provider = Provider::default();
        let now = Utc::now();
        let year = now.year();
        let path = get_filename(year, &provider);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create cache directory");
        }
        let mut hm = HM::new();
        hm.insert(
            (now.day(), now.month()),
            HolidayEntry::official("Main cached holiday".to_string()),
        );
        save(&path, &hm).expect("save cached holidays");

        run_with_args(["cal2"]).expect("invoke should succeed");

        unsafe {
            match previous_home {
                Some(v) => env::set_var("HOME", v),
                None => env::remove_var("HOME"),
            }
            match previous_data {
                Some(v) => env::set_var("XDG_DATA_HOME", v),
                None => env::remove_var("XDG_DATA_HOME"),
            }
            match previous_config {
                Some(v) => env::set_var("XDG_CONFIG_HOME", v),
                None => env::remove_var("XDG_CONFIG_HOME"),
            }
        }
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
