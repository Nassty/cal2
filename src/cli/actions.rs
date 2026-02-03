use crate::HM;
use crate::cli::{ConfigAction, DisplayOptions, Mode, MonthRange, OutputFormat};
use crate::config;
use crate::display_month::DisplayMonth;
use crate::error::Result;
use crate::holidays::{
    HolidayEntry, HolidayKind, Provider, get_filename, get_holidays, load, save,
};
use chrono::{DateTime, Datelike, Utc};
use prettytable::{Cell, Row, Table, format};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::io::{self, Write};
use std::iter::zip;

pub trait ActionEnvironment {
    fn now(&self) -> DateTime<Utc>;
    fn holidays(&self, year: i32) -> Result<HM>;
    fn load(&self, year: i32) -> Result<HM>;
    fn save(&self, year: i32, hm: &HM) -> Result<()>;
    fn print(&self, msg: &str) -> Result<()>;
    fn println(&self, msg: &str) -> Result<()>;
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

    fn holidays(&self, year: i32) -> Result<HM> {
        get_holidays(year, &self.provider)
    }

    fn load(&self, year: i32) -> Result<HM> {
        let fname = get_filename(year, &self.provider);
        let cached = load(&fname)?;
        Ok(cached.unwrap_or_default())
    }

    fn save(&self, year: i32, hm: &HM) -> Result<()> {
        let fname = get_filename(year, &self.provider);
        save(&fname, hm)
    }

    fn print(&self, msg: &str) -> Result<()> {
        let mut stdout = io::stdout();
        stdout.write_all(msg.as_bytes())?;
        stdout.flush()?;
        Ok(())
    }

    fn println(&self, msg: &str) -> Result<()> {
        let mut stdout = io::stdout();
        writeln!(stdout, "{msg}")?;
        stdout.flush()?;
        Ok(())
    }
}

pub fn display<E: ActionEnvironment>(env: &E, mode: Mode) -> Result<()> {
    let now = env.now();
    let today = now.naive_utc().date();
    let options = DisplayOptions {
        highlight_today: true,
        julian: false,
    };
    let hm = env.holidays(now.year())?;
    let calendars: Vec<_> = match mode {
        Mode::Q => {
            let current = DisplayMonth::new(now.month(), now.year(), &hm, options.clone(), today)?;
            vec![current.prev()?, current.clone(), current.next()?]
        }
        Mode::Month => vec![DisplayMonth::new(
            now.month(),
            now.year(),
            &hm,
            options,
            today,
        )?],
        Mode::Year => {
            let mut rows = Vec::with_capacity(12);
            for month in 1..=12 {
                rows.push(DisplayMonth::new(
                    month,
                    now.year(),
                    &hm,
                    options.clone(),
                    today,
                )?);
            }
            rows
        }
    };

    let mut table = Table::new();
    let format = format::FormatBuilder::new().padding(0, 0).build();
    table.set_format(format);
    let headers = calendars
        .iter()
        .map(|x| {
            let mut c = Cell::new(&x.month_name);
            c.align(format::Alignment::CENTER);
            c
        })
        .collect::<Vec<_>>();
    let bodies = calendars
        .iter()
        .map(|x| Cell::new(&x.format()))
        .collect::<Vec<_>>();

    zip(headers.as_slice().chunks(3), bodies.as_slice().chunks(3)).for_each(|(header, body)| {
        table.add_row(Row::new(header.to_vec()));
        table.add_row(Row::new(body.to_vec()));
    });
    env.print(&table.to_string())
}

pub fn display_range<E: ActionEnvironment>(
    env: &E,
    range: MonthRange,
    options: DisplayOptions,
) -> Result<()> {
    let now = env.now();
    let today = now.naive_utc().date();

    // Collect all months in the range
    let mut months: Vec<(u32, i32)> = Vec::with_capacity(range.count);
    let mut month = range.start_month;
    let mut year = range.start_year;
    for _ in 0..range.count {
        months.push((month, year));
        month += 1;
        if month > 12 {
            month = 1;
            year += 1;
        }
    }

    // Collect all unique years and fetch holidays for each
    let years: Vec<i32> = months.iter().map(|(_, y)| *y).collect();
    let mut holidays_by_year: HashMap<i32, HM> = HashMap::new();
    for y in years {
        if !holidays_by_year.contains_key(&y) {
            holidays_by_year.insert(y, env.holidays(y)?);
        }
    }

    // Build DisplayMonth instances
    let calendars: Vec<_> = months
        .iter()
        .map(|(m, y)| {
            let hm = holidays_by_year.get(y).expect("holidays should exist");
            DisplayMonth::new(*m, *y, hm, options.clone(), today)
        })
        .collect::<Result<Vec<_>>>()?;

    // Format and output
    let mut table = Table::new();
    let format = format::FormatBuilder::new().padding(0, 0).build();
    table.set_format(format);
    let headers = calendars
        .iter()
        .map(|x| {
            let mut c = Cell::new(&x.month_name);
            c.align(format::Alignment::CENTER);
            c
        })
        .collect::<Vec<_>>();
    let bodies = calendars
        .iter()
        .map(|x| Cell::new(&x.format()))
        .collect::<Vec<_>>();

    zip(headers.as_slice().chunks(3), bodies.as_slice().chunks(3)).for_each(|(header, body)| {
        table.add_row(Row::new(header.to_vec()));
        table.add_row(Row::new(body.to_vec()));
    });
    env.print(&table.to_string())
}

pub fn list<E: ActionEnvironment>(env: &E, format: OutputFormat) -> Result<()> {
    let now = env.now();
    let year = now.year();
    let mut holidays: Vec<_> = env.holidays(year)?.into_iter().collect();

    if holidays.is_empty() {
        env.println("No holidays found")?;
        return Ok(());
    }

    holidays.sort_by(|a, b| (a.0.1, a.0.0).cmp(&(b.0.1, b.0.0)));

    match format {
        OutputFormat::Table => {
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
            env.println(&lines.join("\n"))
        }
        OutputFormat::Json => {
            #[derive(serde::Serialize)]
            struct Record {
                date: String,
                name: String,
                kind: String,
            }

            let payload: Vec<Record> = holidays
                .into_iter()
                .map(|((day, month), entry)| Record {
                    date: format!("{year}-{month:02}-{day:02}"),
                    name: entry.name,
                    kind: match entry.kind {
                        HolidayKind::Official => "official",
                        HolidayKind::Custom => "custom",
                    }
                    .to_string(),
                })
                .collect();
            let body = serde_json::to_string_pretty(&payload)?;
            env.println(&body)
        }
        OutputFormat::Markdown => {
            let mut records = Vec::with_capacity(holidays.len());
            let mut width_date = "Date".len();
            let mut width_name = "Name".len();
            let mut width_kind = "Kind".len();
            for ((day, month), entry) in holidays {
                let date = format!("{year}-{month:02}-{day:02}");
                let kind = match entry.kind {
                    HolidayKind::Official => "official".to_string(),
                    HolidayKind::Custom => "custom".to_string(),
                };
                width_date = width_date.max(date.len());
                width_name = width_name.max(entry.name.len());
                width_kind = width_kind.max(kind.len());
                records.push((date, entry.name, kind));
            }

            let mut rows = Vec::with_capacity(records.len() + 2);
            rows.push(format!(
                "| {date:<width_date$} | {name:<width_name$} | {kind:<width_kind$} |",
                date = "Date",
                name = "Name",
                kind = "Kind",
                width_date = width_date,
                width_name = width_name,
                width_kind = width_kind,
            ));
            rows.push(format!(
                "| {date:-<width_date$} | {name:-<width_name$} | {kind:-<width_kind$} |",
                date = "",
                name = "",
                kind = "",
                width_date = width_date,
                width_name = width_name,
                width_kind = width_kind,
            ));
            for (date, name, kind) in records {
                rows.push(format!(
                    "| {date:<width_date$} | {name:<width_name$} | {kind:<width_kind$} |",
                    width_date = width_date,
                    width_name = width_name,
                    width_kind = width_kind,
                ));
            }
            env.println(&rows.join("\n"))
        }
    }
}

pub fn add<E: ActionEnvironment>(
    env: &E,
    day: u32,
    month: u32,
    description: Option<String>,
) -> Result<()> {
    let now = env.now();
    let mut hm = env.load(now.year())?;
    if let Entry::Vacant(v) = hm.entry((day, month)) {
        let name = description
            .and_then(|d| {
                let trimmed = d.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
            .unwrap_or_else(|| format!("Custom holiday ({day:02}/{month:02})"));
        v.insert(HolidayEntry::custom(name));
    }
    env.save(now.year(), &hm)?;
    env.println("OK")
}

pub fn delete<E: ActionEnvironment>(env: &E, day: u32, month: u32) -> Result<()> {
    let now = env.now();
    let mut hm = env.load(now.year())?;
    hm.remove(&(day, month));
    env.save(now.year(), &hm)?;
    env.println("OK")
}

pub fn config<E: ActionEnvironment>(env: &E, action: &ConfigAction) -> Result<()> {
    match action {
        ConfigAction::SetCountry { country } => {
            // Validate the country code first
            Provider::from_country(Some(country.clone()))?;

            let mut cfg = config::load_config().unwrap_or_default();
            cfg.default_country = Some(country.trim().to_uppercase());
            config::save_config(&cfg)?;
            env.println(&format!("Default country set to: {}", country.to_uppercase()))
        }
        ConfigAction::ClearCountry => {
            let mut cfg = config::load_config().unwrap_or_default();
            cfg.default_country = None;
            config::save_config(&cfg)?;
            env.println("Default country cleared")
        }
        ConfigAction::Show => {
            let cfg = config::load_config().unwrap_or_default();
            let config_path = config::config_file_path();
            let data_path = config::data_dir();

            let mut lines = vec![
                format!("Config file: {}", config_path.display()),
                format!("Data directory: {}", data_path.display()),
            ];

            if let Some(country) = &cfg.default_country {
                lines.push(format!("Default country: {country}"));
            } else {
                lines.push("Default country: (not set, using built-in default)".to_string());
            }

            env.println(&lines.join("\n"))
        }
    }
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

    impl ActionEnvironment for TestEnvironment {
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
            self.output.borrow_mut().push(format!("{msg}\n"));
            Ok(())
        }
    }

    fn test_now(year: i32, month: u32, day: u32) -> DateTime<Utc> {
        Utc.from_utc_datetime(
            &NaiveDate::from_ymd_opt(year, month, day)
                .expect("valid test date")
                .and_hms_opt(0, 0, 0)
                .expect("valid test time"),
        )
    }

    #[test]
    fn display_writes_calendar_to_environment() {
        let mut holidays = HM::new();
        holidays.insert((1, 1), HolidayEntry::official("New Year's Day".to_string()));
        let env = TestEnvironment::new(test_now(1970, 1, 1)).with_holidays(1970, holidays);

        display(&env, Mode::Month).expect("display should succeed");

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

        display(&env, Mode::Q).expect("display should succeed");

        let output = env
            .outputs()
            .into_iter()
            .next()
            .expect("expected display output");
        assert!(output.contains("December 1969"));
        assert!(output.contains("February 1970"));
    }

    #[test]
    fn display_mode_year_includes_all_months() {
        let env = TestEnvironment::new(test_now(1970, 6, 1));

        display(&env, Mode::Year).expect("display should succeed");

        let output = env
            .outputs()
            .into_iter()
            .next()
            .expect("expected display output");
        assert!(output.contains("January 1970"));
        assert!(output.contains("December 1970"));
    }

    #[test]
    fn list_prints_sorted_holidays_with_kind() {
        let mut holidays = HM::new();
        holidays.insert((1, 1), HolidayEntry::official("New Year's Day".to_string()));
        holidays.insert((24, 12), HolidayEntry::custom("Family dinner".to_string()));
        let env = TestEnvironment::new(test_now(2024, 6, 1)).with_holidays(2024, holidays);

        list(&env, OutputFormat::Table).expect("list should succeed");

        let outputs = env.outputs();
        assert_eq!(outputs.len(), 1);
        assert!(outputs[0].starts_with("2024-01-01"));
        assert!(outputs[0].contains("New Year's Day [official]"));
        assert!(outputs[0].contains("Family dinner [custom]"));
    }

    #[test]
    fn list_sorts_multiple_days_in_same_month() {
        let mut holidays = HM::new();
        holidays.insert((10, 5), HolidayEntry::official("Later Holiday".to_string()));
        holidays.insert(
            (1, 5),
            HolidayEntry::official("Earlier Holiday".to_string()),
        );
        let env = TestEnvironment::new(test_now(2024, 5, 1)).with_holidays(2024, holidays);

        list(&env, OutputFormat::Table).expect("list should succeed");

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

        list(&env, OutputFormat::Table).expect("list should succeed");

        assert_eq!(env.outputs(), vec!["No holidays found\n".to_string()]);
    }

    #[test]
    fn list_outputs_json() {
        let mut holidays = HM::new();
        holidays.insert((1, 1), HolidayEntry::official("New Year's Day".to_string()));
        let env = TestEnvironment::new(test_now(2024, 6, 1)).with_holidays(2024, holidays);

        list(&env, OutputFormat::Json).expect("list should succeed");

        let outputs = env.outputs();
        assert_eq!(outputs.len(), 1);
        let json = outputs[0].trim();
        let value: serde_json::Value = serde_json::from_str(json).expect("valid json output");
        assert!(value.is_array());
        assert_eq!(value[0]["name"], "New Year's Day");
    }

    #[test]
    fn list_outputs_markdown() {
        let mut holidays = HM::new();
        holidays.insert((1, 1), HolidayEntry::official("New Year's Day".to_string()));
        let env = TestEnvironment::new(test_now(2024, 6, 1)).with_holidays(2024, holidays);

        list(&env, OutputFormat::Markdown).expect("list should succeed");

        let outputs = env.outputs();
        assert_eq!(outputs.len(), 1);
        let markdown = outputs[0].trim();
        let mut lines = markdown.lines();
        let header = lines.next().expect("header row");
        assert!(header.contains("Date") && header.contains("Name") && header.contains("Kind"));
        let separator = lines.next().expect("separator row");
        assert!(separator.contains("-") && separator.starts_with('|'));
        let data = lines.next().expect("data row");
        let cells: Vec<_> = data.split('|').map(|c| c.trim()).collect();
        assert!(cells.contains(&"2024-01-01"));
        assert!(cells.contains(&"New Year's Day"));
        assert!(cells.contains(&"official"));
    }

    #[test]
    fn add_stores_holiday_and_prints_ok() {
        let env = TestEnvironment::new(test_now(2024, 5, 1));

        add(&env, 24, 12, None).expect("add should succeed");

        let stored = env.stored(2024).expect("holiday map stored");
        let entry = stored
            .get(&(24, 12))
            .expect("custom holiday should be inserted");
        assert_eq!(entry.kind, HolidayKind::Custom);
        assert!(entry.name.contains("Custom holiday"));
        assert_eq!(env.outputs(), vec!["OK\n".to_string()]);
    }

    #[test]
    fn add_uses_provided_description_when_present() {
        let env = TestEnvironment::new(test_now(2024, 5, 1));

        add(
            &env,
            2,
            7,
            Some("  Family gathering  ".to_string()),
        )
        .expect("add should succeed");

        let stored = env.stored(2024).expect("holiday map stored");
        let entry = stored
            .get(&(2, 7))
            .expect("custom holiday should be inserted");
        assert_eq!(entry.kind, HolidayKind::Custom);
        assert_eq!(entry.name, "Family gathering");
    }

    #[test]
    fn add_does_not_override_existing_official_holiday() {
        let mut store = HM::new();
        store.insert((1, 5), HolidayEntry::official("Labour Day".to_string()));
        let env = TestEnvironment::new(test_now(2024, 5, 1)).with_store(2024, store);

        add(&env, 1, 5, None).expect("add should succeed");

        let stored = env.stored(2024).expect("holiday map stored");
        let entry = stored.get(&(1, 5)).expect("holiday should remain present");
        assert_eq!(entry.kind, HolidayKind::Official);
        assert_eq!(entry.name, "Labour Day");
    }

    #[test]
    fn delete_removes_holiday_and_prints_ok() {
        let mut store = HM::new();
        store.insert((1, 1), HolidayEntry::official("New Year's Day".to_string()));
        store.insert((24, 12), HolidayEntry::custom("Family dinner".to_string()));
        let env = TestEnvironment::new(test_now(2024, 5, 1)).with_store(2024, store);

        delete(&env, 24, 12).expect("delete should succeed");

        let stored = env.stored(2024).expect("holiday map stored");
        assert!(!stored.contains_key(&(24, 12)));
        assert_eq!(env.outputs(), vec!["OK\n".to_string()]);
    }

    #[test]
    #[serial]
    fn real_environment_roundtrip_uses_cache() {
        let temp_dir = {
            let mut path = std::env::temp_dir();
            let nanos = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos();
            path.push(format!("cal2-home-real-env-{nanos}"));
            fs::create_dir_all(&path).expect("create temp dir");
            path
        };

        let previous_home = std::env::var("HOME").ok();
        let previous_data = std::env::var("XDG_DATA_HOME").ok();
        unsafe {
            std::env::set_var("HOME", &temp_dir);
            std::env::set_var("XDG_DATA_HOME", temp_dir.join("data"));
        }

        let provider = Provider::default();
        let year = 2042;
        let path = get_filename(year, &provider);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create cache directory");
        }
        let mut hm = HM::new();
        hm.insert((4, 3), HolidayEntry::official("Cache Test".to_string()));

        let env = RealEnvironment::new(provider);
        env.save(year, &hm).expect("save cache");

        let loaded = env.load(year).expect("load cache");
        assert_eq!(loaded, hm);

        let holidays = env.holidays(year).expect("holidays should load");
        assert_eq!(holidays, hm);

        env.print("noop").expect("print works");
        env.println("noop").expect("println works");

        unsafe {
            match previous_home {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
            match previous_data {
                Some(v) => std::env::set_var("XDG_DATA_HOME", v),
                None => std::env::remove_var("XDG_DATA_HOME"),
            }
        }
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
