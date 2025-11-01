mod actions;

use clap::{Parser, Subcommand, ValueEnum};

use crate::error::Result;
use crate::holidays::Provider;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Args {
    #[arg(long, value_name = "COUNTRY", global = true)]
    pub country: Option<String>,

    #[command(subcommand)]
    pub action: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Add { day: u32, month: u32 },
    Delete { day: u32, month: u32 },
    List,
    Display { mode: Option<Mode> },
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum Mode {
    Q,
    Month,
    Year,
}

impl Args {
    pub fn invoke(&self) -> Result<()> {
        let provider = Provider::from_country(self.country.clone())?;
        let env = actions::RealEnvironment::new(provider);
        self.dispatch(&env)
    }

    fn dispatch<E: actions::ActionEnvironment>(&self, env: &E) -> Result<()> {
        match self.action {
            Some(Commands::Delete { day, month }) => actions::delete(env, day, month),
            Some(Commands::Add { day, month }) => actions::add(env, day, month),
            Some(Commands::Display { mode }) => actions::display(env, mode.unwrap_or(Mode::Q)),
            Some(Commands::List) => actions::list(env),
            None => actions::display(env, Mode::Q),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HM;
    use crate::cli::actions::ActionEnvironment;
    use crate::holidays::{HolidayEntry, HolidayKind, Provider, get_filename, save};
    use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
    use serial_test::serial;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::SystemTime;

    struct RecordingEnv {
        now: DateTime<Utc>,
        holidays: RefCell<HashMap<i32, HM>>,
        output: RefCell<Vec<String>>,
        store: RefCell<HashMap<i32, HM>>,
    }

    impl RecordingEnv {
        fn new(now: DateTime<Utc>) -> Self {
            Self {
                now,
                holidays: RefCell::new(HashMap::new()),
                output: RefCell::new(Vec::new()),
                store: RefCell::new(HashMap::new()),
            }
        }

        fn with_holidays(self, year: i32, hm: HM) -> Self {
            self.holidays.borrow_mut().insert(year, hm);
            self
        }

        fn outputs(&self) -> Vec<String> {
            self.output.borrow().clone()
        }

        fn stored(&self, year: i32) -> Option<HM> {
            self.store.borrow().get(&year).cloned()
        }
    }

    impl ActionEnvironment for RecordingEnv {
        fn now(&self) -> DateTime<Utc> {
            self.now
        }

        fn holidays(&self, year: i32) -> Result<HM> {
            Ok(self
                .holidays
                .borrow()
                .get(&year)
                .cloned()
                .unwrap_or_default())
        }

        fn load(&self, year: i32) -> Result<HM> {
            Ok(self.store.borrow().get(&year).cloned().unwrap_or_default())
        }

        fn save(&self, year: i32, hm: &HM) -> Result<()> {
            self.store.borrow_mut().insert(year, hm.clone());
            Ok(())
        }

        fn print(&self, msg: &str) -> Result<()> {
            self.output.borrow_mut().push(msg.to_string());
            Ok(())
        }

        fn println(&self, msg: &str) -> Result<()> {
            self.output.borrow_mut().push(msg.to_string());
            Ok(())
        }
    }

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

    fn jan_first(year: i32) -> DateTime<Utc> {
        Utc.from_utc_datetime(
            &NaiveDate::from_ymd_opt(year, 1, 1)
                .expect("valid date")
                .and_hms_opt(0, 0, 0)
                .expect("valid time"),
        )
    }

    #[test]
    fn dispatch_defaults_to_quarter_display() {
        let mut hm = HashMap::new();
        hm.insert((1, 1), HolidayEntry::official("New Year's Day".to_string()));
        let env = RecordingEnv::new(jan_first(2024)).with_holidays(2024, hm);
        let args = Args {
            country: None,
            action: None,
        };

        args.dispatch(&env).expect("dispatch succeeds");

        let outputs = env.outputs();
        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].contains("January 2024"));
    }

    #[test]
    fn dispatch_list_invokes_list_handler() {
        let env = RecordingEnv::new(jan_first(2024));
        let args = Args {
            country: None,
            action: Some(Commands::List),
        };

        args.dispatch(&env).expect("dispatch succeeds");

        assert_eq!(env.outputs(), vec!["No holidays found".to_string()]);
    }

    #[test]
    fn dispatch_display_forwards_mode() {
        let env = RecordingEnv::new(jan_first(2024));
        let args = Args {
            country: None,
            action: Some(Commands::Display {
                mode: Some(Mode::Year),
            }),
        };

        args.dispatch(&env).expect("dispatch succeeds");

        let outputs = env.outputs();
        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].contains("December 2024"));
    }

    #[test]
    fn dispatch_add_forwards_to_actions() {
        let env = RecordingEnv::new(jan_first(2024));
        let args = Args {
            country: None,
            action: Some(Commands::Add { day: 1, month: 5 }),
        };

        args.dispatch(&env).expect("dispatch succeeds");

        let stored = env.stored(2024).expect("expected stored holidays");
        let entry = stored
            .get(&(1, 5))
            .expect("expected entry for added holiday");
        assert_eq!(entry.kind, HolidayKind::Custom);
    }

    #[test]
    #[serial]
    fn invoke_uses_real_environment_with_cache() {
        let _home = TempHome::new("invoke");
        let provider = Provider::default();
        let year = Utc::now().year();
        let fname = get_filename(year, &provider);
        if let Some(parent) = Path::new(&fname).parent() {
            fs::create_dir_all(parent).expect("create cache directory");
        }
        let mut hm = HM::new();
        hm.insert(
            (Utc::now().day(), Utc::now().month()),
            HolidayEntry::official("Cached holiday".to_string()),
        );
        save(&fname, &hm).expect("save cached holidays");

        let args = Args {
            country: None,
            action: None,
        };

        args.invoke().expect("invoke should succeed");
    }
}
