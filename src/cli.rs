mod actions;

use chrono::Datelike;
use clap::{Parser, Subcommand, ValueEnum};

use crate::error::Result;
use crate::holidays::Provider;

/// Display options for calendar rendering
#[derive(Clone, Debug, Default)]
pub struct DisplayOptions {
    pub highlight_today: bool,
    pub julian: bool,
}

/// Range of months to display
#[derive(Clone, Debug)]
pub struct MonthRange {
    pub start_month: u32,
    pub start_year: i32,
    pub count: usize,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
#[command(disable_help_flag = true)]
pub struct Args {
    #[arg(long, action = clap::ArgAction::Help)]
    help: Option<bool>,

    #[arg(long, value_name = "COUNTRY", global = true)]
    pub country: Option<String>,

    /// Turn off highlighting of today
    #[arg(short = 'h')]
    pub no_highlight: bool,

    /// Display Julian days (day-of-year 1-365)
    #[arg(short = 'j')]
    pub julian: bool,

    /// Display full year calendar
    #[arg(short = 'y')]
    pub full_year: bool,

    /// Display previous, current, and next month
    #[arg(short = '3')]
    pub three_months: bool,

    /// Display specific month (1-12)
    #[arg(short = 'm', value_name = "MONTH")]
    pub month: Option<u32>,

    /// Display N months after current/specified month
    #[arg(short = 'A', value_name = "NUM")]
    pub months_after: Option<u32>,

    /// Display N months before current/specified month
    #[arg(short = 'B', value_name = "NUM")]
    pub months_before: Option<u32>,

    /// Positional arguments: [[month] year]
    #[arg(value_name = "ARGS")]
    pub positional: Vec<u32>,

    #[command(subcommand)]
    pub action: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Add {
        day: u32,
        month: u32,
        #[arg(long)]
        description: Option<String>,
    },
    Delete {
        day: u32,
        month: u32,
    },
    List {
        #[arg(long, value_enum, default_value_t = OutputFormat::default())]
        format: OutputFormat,
    },
    Display {
        mode: Option<Mode>,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum Mode {
    Q,
    Month,
    Year,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug, Default)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    Markdown,
}

impl Args {
    pub fn invoke(&self) -> Result<()> {
        let provider = Provider::from_country(self.country.clone())?;
        let env = actions::RealEnvironment::new(provider);
        self.dispatch(&env)
    }

    /// Check if any cal-style flags or positional args are provided
    fn has_cal_flags(&self) -> bool {
        self.no_highlight
            || self.julian
            || self.full_year
            || self.three_months
            || self.month.is_some()
            || self.months_after.is_some()
            || self.months_before.is_some()
            || !self.positional.is_empty()
    }

    /// Parse positional arguments [[month] year] into (month, year)
    /// Returns (None, None) if no positional args
    /// Returns (None, Some(year)) for single arg that looks like a year
    /// Returns (Some(month), Some(year)) for two args
    fn parse_positional(&self, _current_month: u32, _current_year: i32) -> (Option<u32>, Option<i32>) {
        match self.positional.as_slice() {
            [] => (None, None),
            [single] => {
                // Numbers 1-12 could be month or year, but 4+ digits or > 12 = year
                if *single > 12 || *single >= 1000 {
                    (None, Some(*single as i32))
                } else {
                    // Ambiguous: treat as month for current year
                    (Some(*single), None)
                }
            }
            [month, year] => (Some(*month), Some(*year as i32)),
            [month, year, ..] => (Some(*month), Some(*year as i32)),
        }
    }

    /// Build DisplayOptions from flags
    fn display_options(&self) -> DisplayOptions {
        DisplayOptions {
            highlight_today: !self.no_highlight,
            julian: self.julian,
        }
    }

    /// Build MonthRange from flags and positional args
    fn month_range(&self, current_month: u32, current_year: i32) -> MonthRange {
        let (pos_month, pos_year) = self.parse_positional(current_month, current_year);

        // Determine base month and year
        let base_month = self.month.or(pos_month).unwrap_or(current_month);
        let base_year = pos_year.unwrap_or(current_year);

        // Calculate range based on flags
        if self.full_year {
            // Full year: 12 months starting from January
            MonthRange {
                start_month: 1,
                start_year: base_year,
                count: 12,
            }
        } else if self.three_months {
            // Three months: prev, current, next (centered on base month)
            let months_before = self.months_before.unwrap_or(1);
            let months_after = self.months_after.unwrap_or(1);
            let (start_month, start_year) =
                Self::offset_month(base_month, base_year, -(months_before as i32));
            MonthRange {
                start_month,
                start_year,
                count: (months_before + 1 + months_after) as usize,
            }
        } else if self.months_before.is_some() || self.months_after.is_some() {
            // -A and/or -B without -3
            let months_before = self.months_before.unwrap_or(0);
            let months_after = self.months_after.unwrap_or(0);
            let (start_month, start_year) =
                Self::offset_month(base_month, base_year, -(months_before as i32));
            MonthRange {
                start_month,
                start_year,
                count: (months_before + 1 + months_after) as usize,
            }
        } else {
            // Single month
            MonthRange {
                start_month: base_month,
                start_year: base_year,
                count: 1,
            }
        }
    }

    /// Offset a month by a number of months (positive or negative)
    fn offset_month(month: u32, year: i32, offset: i32) -> (u32, i32) {
        let total_months = (year * 12 + month as i32 - 1) + offset;
        let new_year = total_months.div_euclid(12);
        let new_month = (total_months.rem_euclid(12) + 1) as u32;
        (new_month, new_year)
    }

    fn dispatch<E: actions::ActionEnvironment>(&self, env: &E) -> Result<()> {
        match self.action.as_ref() {
            Some(Commands::Delete { day, month }) => actions::delete(env, *day, *month),
            Some(Commands::Add {
                day,
                month,
                description,
            }) => actions::add(env, *day, *month, description.clone()),
            Some(Commands::Display { mode }) => actions::display(env, (*mode).unwrap_or(Mode::Q)),
            Some(Commands::List { format }) => actions::list(env, *format),
            None => {
                if self.has_cal_flags() {
                    let now = env.now();
                    let range = self.month_range(now.month(), now.year());
                    let options = self.display_options();
                    actions::display_range(env, range, options)
                } else {
                    actions::display(env, Mode::Q)
                }
            }
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

    fn default_args() -> Args {
        Args {
            help: None,
            country: None,
            no_highlight: false,
            julian: false,
            full_year: false,
            three_months: false,
            month: None,
            months_after: None,
            months_before: None,
            positional: vec![],
            action: None,
        }
    }

    #[test]
    fn dispatch_defaults_to_quarter_display() {
        let mut hm = HashMap::new();
        hm.insert((1, 1), HolidayEntry::official("New Year's Day".to_string()));
        let env = RecordingEnv::new(jan_first(2024)).with_holidays(2024, hm);
        let args = default_args();

        args.dispatch(&env).expect("dispatch succeeds");

        let outputs = env.outputs();
        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].contains("January 2024"));
    }

    #[test]
    fn dispatch_list_invokes_list_handler() {
        let env = RecordingEnv::new(jan_first(2024));
        let mut args = default_args();
        args.action = Some(Commands::List {
            format: OutputFormat::Table,
        });

        args.dispatch(&env).expect("dispatch succeeds");

        assert_eq!(env.outputs(), vec!["No holidays found".to_string()]);
    }

    #[test]
    fn dispatch_display_forwards_mode() {
        let env = RecordingEnv::new(jan_first(2024));
        let mut args = default_args();
        args.action = Some(Commands::Display {
            mode: Some(Mode::Year),
        });

        args.dispatch(&env).expect("dispatch succeeds");

        let outputs = env.outputs();
        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].contains("December 2024"));
    }

    #[test]
    fn dispatch_add_forwards_to_actions() {
        let env = RecordingEnv::new(jan_first(2024));
        let mut args = default_args();
        args.action = Some(Commands::Add {
            day: 1,
            month: 5,
            description: None,
        });

        args.dispatch(&env).expect("dispatch succeeds");

        let stored = env.stored(2024).expect("expected stored holidays");
        let entry = stored
            .get(&(1, 5))
            .expect("expected entry for added holiday");
        assert_eq!(entry.kind, HolidayKind::Custom);
        assert!(entry.name.contains("Custom holiday"));
    }

    #[test]
    fn dispatch_add_forwards_description() {
        let env = RecordingEnv::new(jan_first(2024));
        let mut args = default_args();
        args.action = Some(Commands::Add {
            day: 6,
            month: 7,
            description: Some("Independence Eve".to_string()),
        });

        args.dispatch(&env).expect("dispatch succeeds");

        let stored = env.stored(2024).expect("expected stored holidays");
        let entry = stored
            .get(&(6, 7))
            .expect("expected entry for added holiday");
        assert_eq!(entry.kind, HolidayKind::Custom);
        assert_eq!(entry.name, "Independence Eve");
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

        let args = default_args();

        args.invoke().expect("invoke should succeed");
    }

    #[test]
    fn dispatch_full_year_flag_uses_display_range() {
        let env = RecordingEnv::new(jan_first(2024)).with_holidays(2024, HashMap::new());
        let mut args = default_args();
        args.full_year = true;

        args.dispatch(&env).expect("dispatch succeeds");

        let outputs = env.outputs();
        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].contains("January 2024"));
        assert!(outputs[0].contains("December 2024"));
    }

    #[test]
    fn dispatch_three_months_flag_uses_display_range() {
        let env = RecordingEnv::new(jan_first(2024))
            .with_holidays(2023, HashMap::new())
            .with_holidays(2024, HashMap::new());
        let mut args = default_args();
        args.three_months = true;

        args.dispatch(&env).expect("dispatch succeeds");

        let outputs = env.outputs();
        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].contains("December 2023"));
        assert!(outputs[0].contains("January 2024"));
        assert!(outputs[0].contains("February 2024"));
    }

    #[test]
    fn dispatch_month_flag_displays_specific_month() {
        let env = RecordingEnv::new(jan_first(2024)).with_holidays(2024, HashMap::new());
        let mut args = default_args();
        args.month = Some(6);

        args.dispatch(&env).expect("dispatch succeeds");

        let outputs = env.outputs();
        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].contains("June 2024"));
    }

    #[test]
    fn dispatch_positional_year_displays_that_year() {
        let env = RecordingEnv::new(jan_first(2024)).with_holidays(2025, HashMap::new());
        let mut args = default_args();
        args.positional = vec![2025];

        args.dispatch(&env).expect("dispatch succeeds");

        let outputs = env.outputs();
        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].contains("January 2025"));
    }

    #[test]
    fn dispatch_positional_month_year_displays_specific_month() {
        let env = RecordingEnv::new(jan_first(2024)).with_holidays(2025, HashMap::new());
        let mut args = default_args();
        args.positional = vec![6, 2025];

        args.dispatch(&env).expect("dispatch succeeds");

        let outputs = env.outputs();
        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].contains("June 2025"));
    }

    #[test]
    fn month_range_with_months_after() {
        let mut args = default_args();
        args.months_after = Some(2);
        let range = args.month_range(3, 2024);
        assert_eq!(range.start_month, 3);
        assert_eq!(range.start_year, 2024);
        assert_eq!(range.count, 3);
    }

    #[test]
    fn month_range_with_months_before() {
        let mut args = default_args();
        args.months_before = Some(2);
        let range = args.month_range(3, 2024);
        assert_eq!(range.start_month, 1);
        assert_eq!(range.start_year, 2024);
        assert_eq!(range.count, 3);
    }

    #[test]
    fn offset_month_handles_year_boundaries() {
        assert_eq!(Args::offset_month(1, 2024, -1), (12, 2023));
        assert_eq!(Args::offset_month(12, 2024, 1), (1, 2025));
        assert_eq!(Args::offset_month(6, 2024, -12), (6, 2023));
    }
}
