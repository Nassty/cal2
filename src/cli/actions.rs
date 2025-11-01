use crate::HM;
use crate::cli::Mode;
use crate::display_month::DisplayMonth;
use crate::holidays::{
    HolidayEntry, HolidayKind, Provider, get_filename, get_holidays, load, save,
};
use chrono::{DateTime, Datelike, Utc};
use prettytable::{Cell, Row, Table, format};
use std::collections::hash_map::Entry;
use std::iter::zip;

pub trait ActionEnvironment {
    fn now(&self) -> DateTime<Utc>;
    fn holidays(&self, year: i32) -> HM;
    fn load(&self, year: i32) -> HM;
    fn save(&self, year: i32, hm: &HM);
    fn print(&self, msg: &str);
    fn println(&self, msg: &str);
}

#[derive(Default)]
pub struct RealEnvironment {
    provider: Provider,
}

impl RealEnvironment {
    pub fn new(provider: Provider) -> Self {
        Self { provider }
    }
}

impl ActionEnvironment for RealEnvironment {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn holidays(&self, year: i32) -> HM {
        get_holidays(year, &self.provider)
    }

    fn load(&self, year: i32) -> HM {
        let fname = get_filename(year, &self.provider);
        load(&fname).unwrap_or_default()
    }

    fn save(&self, year: i32, hm: &HM) {
        let fname = get_filename(year, &self.provider);
        save(&fname, hm);
    }

    fn print(&self, msg: &str) {
        print!("{msg}");
    }

    fn println(&self, msg: &str) {
        println!("{msg}");
    }
}

pub fn display<E: ActionEnvironment>(env: &E, mode: Mode) {
    let now = env.now();
    let hm = env.holidays(now.year());
    let dm = DisplayMonth::new(now.month(), now.year(), &hm);
    let cals = match mode {
        Mode::Q => Vec::from([dm.prev(), dm.clone(), dm.next()]),
        Mode::Month => Vec::from([dm]),
        Mode::Year => (1..=12)
            .map(|x| DisplayMonth::new(x, now.year(), &hm))
            .collect(),
    };
    let mut table = Table::new();
    let format = format::FormatBuilder::new().padding(0, 0).build();
    table.set_format(format);
    let headers = cals
        .iter()
        .map(|x| {
            let mut c = Cell::new(&x.month_name);
            c.align(format::Alignment::CENTER);
            c
        })
        .collect::<Vec<_>>();
    let bodies = cals
        .iter()
        .map(|x| Cell::new(&x.format()))
        .collect::<Vec<_>>();

    zip(headers.as_slice().chunks(3), bodies.as_slice().chunks(3)).for_each(|(header, body)| {
        table.add_row(Row::new(header.to_vec()));
        table.add_row(Row::new(body.to_vec()));
    });
    env.print(&table.to_string());
}

pub fn list<E: ActionEnvironment>(env: &E) {
    let now = env.now();
    let year = now.year();
    let mut holidays: Vec<_> = env.holidays(year).into_iter().collect();

    if holidays.is_empty() {
        env.println("No holidays found");
        return;
    }

    holidays.sort_by(|a, b| (a.0.1, a.0.0).cmp(&(b.0.1, b.0.0)));

    let lines: Vec<String> = holidays
        .into_iter()
        .map(|((day, month), entry)| {
            let date = format!("{year}-{month:02}-{day:02}");
            let kind = match entry.kind {
                HolidayKind::Official => "official",
                HolidayKind::Custom => "custom",
            };
            format!("{date}  {} [{kind}]", entry.name)
        })
        .collect();

    env.println(&lines.join("\n"));
}

pub fn add<E: ActionEnvironment>(env: &E, day: u32, month: u32) {
    let now = env.now();
    let mut hm = env.load(now.year());
    match hm.entry((day, month)) {
        Entry::Occupied(_) => { /* keep existing */ }
        Entry::Vacant(v) => {
            let name = format!("Custom holiday ({day:02}/{month:02})");
            v.insert(HolidayEntry::custom(name));
        }
    }
    env.save(now.year(), &hm);
    env.println("OK");
}

pub fn delete<E: ActionEnvironment>(env: &E, day: u32, month: u32) {
    let now = env.now();
    let mut hm = env.load(now.year());
    hm.remove(&(day, month));
    env.save(now.year(), &hm);
    env.println("OK");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Mode;
    use crate::holidays::{HolidayEntry, Provider, get_filename};
    use chrono::{NaiveDate, TimeZone};
    use serial_test::serial;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::SystemTime;

    struct TestEnvironment {
        now: DateTime<Utc>,
        holidays: RefCell<HashMap<i32, HM>>,
        store: RefCell<HashMap<i32, HM>>,
        output: RefCell<Vec<String>>,
    }

    impl TestEnvironment {
        fn new(date: DateTime<Utc>) -> Self {
            Self {
                now: date,
                holidays: RefCell::new(HashMap::new()),
                store: RefCell::new(HashMap::new()),
                output: RefCell::new(Vec::new()),
            }
        }

        fn with_holidays(self, year: i32, hm: HM) -> Self {
            self.holidays.borrow_mut().insert(year, hm);
            self
        }

        fn with_store(self, year: i32, hm: HM) -> Self {
            self.store.borrow_mut().insert(year, hm);
            self
        }

        fn outputs(&self) -> Vec<String> {
            self.output.borrow().clone()
        }

        fn stored(&self, year: i32) -> Option<HM> {
            self.store.borrow().get(&year).cloned()
        }
    }

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
            unsafe { std::env::set_var("HOME", &path) };
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

    impl ActionEnvironment for TestEnvironment {
        fn now(&self) -> DateTime<Utc> {
            self.now
        }

        fn holidays(&self, year: i32) -> HM {
            self.holidays
                .borrow()
                .get(&year)
                .cloned()
                .unwrap_or_default()
        }

        fn load(&self, year: i32) -> HM {
            self.store.borrow().get(&year).cloned().unwrap_or_default()
        }

        fn save(&self, year: i32, hm: &HM) {
            self.store.borrow_mut().insert(year, hm.clone());
        }

        fn print(&self, msg: &str) {
            self.output.borrow_mut().push(msg.to_string());
        }

        fn println(&self, msg: &str) {
            self.output.borrow_mut().push(msg.to_string());
        }
    }

    fn test_now(year: i32, month: u32, day: u32) -> DateTime<Utc> {
        Utc.from_utc_datetime(
            &NaiveDate::from_ymd_opt(year, month, day)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
        )
    }

    #[test]
    fn display_writes_calendar_to_environment() {
        let mut holidays = HashMap::new();
        holidays.insert((1, 1), HolidayEntry::official("New Year's Day".to_string()));
        let env = TestEnvironment::new(test_now(1970, 1, 1)).with_holidays(1970, holidays);

        display(&env, Mode::Month);

        let outputs = env.outputs();
        assert_eq!(outputs.len(), 1);
        assert!(
            outputs[0].contains("January 1970"),
            "expected month header in output"
        );
    }

    #[test]
    fn display_mode_q_includes_prev_and_next_months() {
        let env = TestEnvironment::new(test_now(1970, 1, 1));

        display(&env, Mode::Q);

        let output = env
            .outputs()
            .into_iter()
            .next()
            .expect("expected display output");
        assert!(
            output.contains("December 1969"),
            "quarter view should include previous month"
        );
        assert!(
            output.contains("February 1970"),
            "quarter view should include next month"
        );
    }

    #[test]
    fn display_mode_year_includes_all_months() {
        let env = TestEnvironment::new(test_now(1970, 6, 1));

        display(&env, Mode::Year);

        let output = env
            .outputs()
            .into_iter()
            .next()
            .expect("expected display output");
        assert!(
            output.contains("January 1970") && output.contains("December 1970"),
            "year view should include both January and December"
        );
    }

    #[test]
    fn list_prints_sorted_holidays_with_kind() {
        let mut holidays = HashMap::new();
        holidays.insert((1, 1), HolidayEntry::official("New Year's Day".to_string()));
        holidays.insert((24, 12), HolidayEntry::custom("Family dinner".to_string()));
        let env = TestEnvironment::new(test_now(2024, 6, 1)).with_holidays(2024, holidays);

        list(&env);

        let outputs = env.outputs();
        assert_eq!(outputs.len(), 1);
        assert!(
            outputs[0].starts_with("2024-01-01"),
            "expected chronological order, got {}",
            outputs[0]
        );
        assert!(
            outputs[0].contains("New Year's Day [official]"),
            "expected official tag in output"
        );
        assert!(
            outputs[0].contains("Family dinner [custom]"),
            "expected custom tag in output"
        );
    }

    #[test]
    fn list_sorts_multiple_days_in_same_month() {
        let mut holidays = HashMap::new();
        holidays.insert((10, 5), HolidayEntry::official("Later Holiday".to_string()));
        holidays.insert(
            (1, 5),
            HolidayEntry::official("Earlier Holiday".to_string()),
        );
        let env = TestEnvironment::new(test_now(2024, 5, 1)).with_holidays(2024, holidays);

        list(&env);

        let output = env
            .outputs()
            .into_iter()
            .next()
            .expect("expected list output");
        let mut lines = output.lines();
        assert_eq!(lines.next(), Some("2024-05-01  Earlier Holiday [official]"));
        assert_eq!(lines.next(), Some("2024-05-10  Later Holiday [official]"));
    }

    #[test]
    fn list_informs_when_no_holidays_available() {
        let env = TestEnvironment::new(test_now(2024, 6, 1));

        list(&env);

        assert_eq!(env.outputs(), vec!["No holidays found".to_string()]);
    }

    #[test]
    fn add_stores_holiday_and_prints_ok() {
        let env = TestEnvironment::new(test_now(2024, 5, 1));

        add(&env, 24, 12);

        let stored = env.stored(2024).expect("holiday map stored");
        let entry = stored
            .get(&(24, 12))
            .expect("custom holiday should be inserted");
        assert_eq!(entry.kind, HolidayKind::Custom);
        assert!(entry.name.contains("Custom holiday"));
        assert_eq!(env.outputs(), vec!["OK".to_string()]);
    }

    #[test]
    fn add_does_not_override_existing_official_holiday() {
        let mut store = HashMap::new();
        store.insert((1, 5), HolidayEntry::official("Labour Day".to_string()));
        let env = TestEnvironment::new(test_now(2024, 5, 1)).with_store(2024, store);

        add(&env, 1, 5);

        let stored = env.stored(2024).expect("holiday map stored");
        let entry = stored.get(&(1, 5)).expect("holiday should remain present");
        assert_eq!(entry.kind, HolidayKind::Official);
        assert_eq!(entry.name, "Labour Day");
    }

    #[test]
    #[serial]
    fn real_environment_roundtrip_uses_cache() {
        let _home = unsafe { TempHome::new("real-env") };
        let provider = Provider::default();
        let year = 2042;
        let fname = get_filename(year, &provider);
        if let Some(parent) = Path::new(&fname).parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut hm = HM::new();
        hm.insert((4, 3), HolidayEntry::official("Cache Test".to_string()));

        let env = RealEnvironment::new(provider);
        env.save(year, &hm);

        let loaded = env.load(year);
        assert_eq!(loaded, hm);

        let holidays = env.holidays(year);
        assert_eq!(holidays, hm);

        env.print("noop");
        env.println("noop");
    }

    #[test]
    fn delete_removes_holiday_and_prints_ok() {
        let mut store = HashMap::new();
        store.insert((1, 1), HolidayEntry::official("New Year's Day".to_string()));
        store.insert((24, 12), HolidayEntry::custom("Family dinner".to_string()));
        let env = TestEnvironment::new(test_now(2024, 5, 1)).with_store(2024, store);

        delete(&env, 24, 12);

        let stored = env.stored(2024).expect("holiday map stored");
        assert!(!stored.contains_key(&(24, 12)));
        assert_eq!(env.outputs(), vec!["OK".to_string()]);
    }
}
