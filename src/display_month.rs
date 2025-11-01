use crate::{
    HM,
    error::{CalError, Result},
};
use chrono::{self, Datelike, Days, Month, NaiveDate, Weekday};
use colored::Colorize;
use prettytable::{Cell, Row, Table, format};

#[derive(Clone)]
pub struct DisplayMonth<'a> {
    pub month: u32,
    pub month_name: String,
    pub year: i32,
    first_day: NaiveDate,
    last_day: NaiveDate,
    hm: &'a HM,
}

impl<'a> DisplayMonth<'a> {
    pub fn new(month: u32, year: i32, hm: &'a HM) -> Result<Self> {
        let first_day = NaiveDate::from_ymd_opt(year, month, 1)
            .ok_or_else(|| CalError::InvalidDate(format!("invalid month {month}")))?;
        let last_day = NaiveDate::from_ymd_opt(year, month + 1, 1)
            .or_else(|| NaiveDate::from_ymd_opt(year + 1, 1, 1))
            .and_then(|d| d.pred_opt())
            .ok_or_else(|| CalError::InvalidDate(format!("invalid month {month}")))?;
        let month_name = Month::try_from(month as u8)
            .map(|m| format!("{} {year}", m.name()))
            .map_err(|_| CalError::InvalidDate(format!("invalid month {month}")))?;

        Ok(Self {
            month,
            year,
            first_day,
            last_day,
            month_name,
            hm,
        })
    }

    pub fn next(&self) -> Result<Self> {
        let next_month = (self.month % 12) + 1;
        let year = if next_month > self.month {
            self.year
        } else {
            self.year + 1
        };
        Self::new(next_month, year, self.hm)
    }

    pub fn prev(&self) -> Result<Self> {
        let prev_month = if self.month == 1 { 12 } else { self.month - 1 };
        let year = if prev_month < self.month {
            self.year
        } else {
            self.year - 1
        };
        Self::new(prev_month, year, self.hm)
    }

    pub fn get_matrix(&self) -> Vec<Vec<String>> {
        let today = chrono::Utc::now().naive_local().date();
        let mut curr_day = self.first_day;
        let first_index = self.first_day.weekday().number_from_monday();
        let weekends = [Weekday::Sat, Weekday::Sun];
        (1..self.last_day.day() + first_index)
            .map(|i| {
                if i < first_index {
                    return None;
                }

                let cr = curr_day;
                if let Some(next_day) = curr_day.checked_add_days(Days::new(1)) {
                    curr_day = next_day;
                }
                let day = cr.day();
                let is_holiday = self.hm.contains_key(&(day, self.month));
                Some((cr, is_holiday))
            })
            .map(|x| match x {
                Some((cr, _)) if cr == today => cr.day().to_string().black().on_white().to_string(),
                Some((cr, _)) if weekends.contains(&cr.weekday()) => {
                    cr.day().to_string().green().to_string()
                }
                Some((cr, true)) => cr.day().to_string().red().to_string(),
                Some((cr, false)) => cr.day().to_string(),
                None => String::new(),
            })
            .collect::<Vec<_>>()
            .chunks(7)
            .map(|x| x.to_vec())
            .collect()
    }

    pub fn format(&self) -> String {
        const WEEKDAYS: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
        let mut table = Table::new();
        let format = format::FormatBuilder::new()
            .column_separator(' ')
            .borders(' ')
            .separators(
                &[format::LinePosition::Top, format::LinePosition::Bottom],
                format::LineSeparator::new(' ', ' ', ' ', ' '),
            )
            .padding(0, 0)
            .build();
        table.set_format(format);
        table.add_row(Row::new(
            WEEKDAYS
                .iter()
                .map(|label| Cell::new(label))
                .collect::<Vec<_>>(),
        ));
        self.get_matrix().iter().for_each(|x| {
            table.add_row(Row::new(x.iter().map(|y: &String| Cell::new(y)).collect()));
        });

        table.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::holidays::HolidayEntry;
    use std::collections::HashMap;

    struct ColorGuard;

    impl ColorGuard {
        fn enable() -> Self {
            colored::control::set_override(true);
            Self
        }
    }

    impl Drop for ColorGuard {
        fn drop(&mut self) {
            colored::control::set_override(false);
        }
    }

    #[test]
    fn prev_from_january_wraps_to_december_previous_year() {
        let hm = HashMap::new();
        let dm = DisplayMonth::new(1, 2024, &hm).expect("valid display month");
        let prev = dm.prev().expect("previous month available");

        assert_eq!(prev.month, 12);
        assert_eq!(prev.year, 2023);
    }

    #[test]
    fn next_from_december_wraps_to_january_next_year() {
        let hm = HashMap::new();
        let dm = DisplayMonth::new(12, 2023, &hm).expect("valid display month");
        let next = dm.next().expect("next month available");

        assert_eq!(next.month, 1);
        assert_eq!(next.year, 2024);
    }

    #[test]
    fn get_matrix_marks_holidays_and_weekends() {
        let _color_guard = ColorGuard::enable();
        let mut hm = HashMap::new();
        hm.insert(
            (6, 1),
            HolidayEntry::custom("Test custom holiday".to_string()),
        );
        let dm = DisplayMonth::new(1, 1970, &hm).expect("valid display month");

        let matrix = dm.get_matrix();
        assert_eq!(matrix.len(), 5);
        assert!(matrix.iter().all(|row| row.len() <= 7));

        assert_eq!(matrix[0][0], "");
        assert_eq!(matrix[0][1], "");
        assert_eq!(matrix[0][2], "");

        let flattened: Vec<&String> = matrix.iter().flat_map(|row| row.iter()).collect();
        let filled_cells = flattened.iter().filter(|cell| !cell.is_empty()).count();
        assert_eq!(filled_cells, 31);

        let holiday_cell = flattened
            .iter()
            .find(|cell| cell.contains('6') && cell.contains('\u{1b}'));
        assert!(
            holiday_cell.is_some(),
            "expected coloured holiday for day 6"
        );
        assert!(
            holiday_cell
                .expect("holiday cell exists")
                .contains("\u{1b}[31m"),
            "holiday cell should be red"
        );

        let weekend_cell = flattened.iter().find(|cell| cell.contains("\u{1b}[32m"));
        assert!(
            weekend_cell.is_some(),
            "expected coloured weekend in matrix"
        );
    }

    #[test]
    fn format_includes_weekday_headers() {
        let _color_guard = ColorGuard::enable();
        let hm = HashMap::new();
        let dm = DisplayMonth::new(1, 2024, &hm).expect("valid display month");

        let formatted = dm.format();
        assert!(formatted.contains("Mo"));
        assert!(formatted.contains("Su"));
    }
}
