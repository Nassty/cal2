use clap::Parser;
use std::{collections::HashMap, ffi::OsString, process};

mod cli;
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
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::SystemTime;

    struct TempHome {
        previous: Option<String>,
        path: PathBuf,
    }

    impl TempHome {
        fn new(label: &str) -> Self {
            let mut path = std::env::temp_dir();
            let nanos = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos();
            path.push(format!("cal2-home-{label}-{nanos}"));
            fs::create_dir_all(&path).expect("create temporary home directory");
            let previous = std::env::var("HOME").ok();
            unsafe {
                std::env::set_var("HOME", &path);
            }
            Self { previous, path }
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            unsafe {
                if let Some(prev) = &self.previous {
                    std::env::set_var("HOME", prev);
                } else {
                    std::env::remove_var("HOME");
                }
            }
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    #[serial]
    fn run_with_args_uses_cached_display() {
        let _home = TempHome::new("main-run");
        let provider = Provider::default();
        let now = Utc::now();
        let year = now.year();
        let fname = get_filename(year, &provider);
        if let Some(parent) = Path::new(&fname).parent() {
            fs::create_dir_all(parent).expect("create cache directory");
        }
        let mut hm = HM::new();
        hm.insert(
            (now.day(), now.month()),
            HolidayEntry::official("Main cached holiday".to_string()),
        );
        save(&fname, &hm).expect("save cached holidays");

        run_with_args(["cal2"]).expect("invoke should succeed");
    }
}
