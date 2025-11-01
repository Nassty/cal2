use std::iter::zip;

use crate::cli::Mode;
use crate::holidays::{get_filename, get_holidays, load, save};
use crate::HM;
use chrono::{DateTime, Datelike, Utc};
use prettytable::{format, Cell, Row, Table};

pub trait ActionEnvironment {
    fn now(&self) -> DateTime<Utc>;
    fn holidays(&self, year: i32) -> HM;
    fn load(&self, year: i32) -> HM;
    fn save(&self, year: i32, hm: &HM);
    fn print(&self, msg: &str);
    fn println(&self, msg: &str);
}

#[derive(Clone, Copy, Default)]
pub struct RealEnvironment;

impl ActionEnvironment for RealEnvironment {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn holidays(&self, year: i32) -> HM {
        get_holidays(year)
    }

    fn load(&self, year: i32) -> HM {
        let fname = get_filename(year);
        load(&fname).unwrap_or_default()
    }

    fn save(&self, year: i32, hm: &HM) {
        let fname = get_filename(year);
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
    let dm = crate::display_month::DisplayMonth::new(now.month(), now.year(), &hm);
    let cals = match mode {
        Mode::Q => Vec::from([dm.prev(), dm.clone(), dm.next()]),
        Mode::Month => Vec::from([dm]),
        Mode::Year => (1..=12)
            .map(|x| crate::display_month::DisplayMonth::new(x, now.year(), &hm))
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

pub fn add<E: ActionEnvironment>(env: &E, day: u32, month: u32) {
    let now = env.now();
    let mut hm = env.load(now.year());
    hm.insert((day, month), true);
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
    use chrono::{NaiveDate, TimeZone};
    use std::cell::RefCell;
    use std::collections::HashMap;

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
            self.output.borrow_mut().push(format!("{msg}\n"));
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
        let mut holidays = HM::new();
        holidays.insert((1, 1), true);
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
    fn add_stores_holiday_and_prints_ok() {
        let env = TestEnvironment::new(test_now(2024, 5, 1));

        add(&env, 6, 1);

        let stored = env.stored(2024).expect("holiday map saved");
        assert_eq!(stored.get(&(6, 1)), Some(&true));
        assert_eq!(env.outputs(), vec!["OK\n".to_string()]);
    }

    #[test]
    fn delete_removes_holiday_and_prints_ok() {
        let mut hm = HM::new();
        hm.insert((6, 1), true);
        let env = TestEnvironment::new(test_now(2024, 5, 1)).with_store(2024, hm);

        delete(&env, 6, 1);

        let stored = env.stored(2024).expect("holiday map saved");
        assert!(!stored.contains_key(&(6, 1)));
        assert_eq!(env.outputs(), vec!["OK\n".to_string()]);
    }
}
