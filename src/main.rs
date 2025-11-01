use clap::Parser;
use std::{collections::HashMap, ffi::OsString};

mod cli;
mod display_month;
mod holidays;

use holidays::HolidayEntry;

type HM = HashMap<(u32, u32), HolidayEntry>;

pub fn run_with_args<I, T>(args: I)
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let args = cli::Args::parse_from(args);
    args.invoke();
}

fn main() {
    run_with_args(std::env::args());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HM;
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
        unsafe fn new(label: &str) -> Self {
            let mut path = std::env::temp_dir();
            let nanos = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            path.push(format!("cal2-home-{label}-{nanos}"));
            fs::create_dir_all(&path).unwrap();
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
        let _home = unsafe { TempHome::new("main-run") };
        let provider = Provider::default();
        let year = Utc::now().year();
        let fname = get_filename(year, &provider);
        if let Some(parent) = Path::new(&fname).parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut hm: HM = HashMap::new();
        hm.insert(
            (Utc::now().day(), Utc::now().month()),
            HolidayEntry::official("Main cached holiday".to_string()),
        );
        save(&fname, &hm);

        run_with_args(["cal2"]);
    }
}
