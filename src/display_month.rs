use crate::{
    HM,
    cli::DisplayOptions,
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
    options: DisplayOptions,
    today: NaiveDate,
}

impl<'a> DisplayMonth<'a> {
    pub fn new(
        month: u32,
        year: i32,
        hm: &'a HM,
        options: DisplayOptions,
        today: NaiveDate,
    ) -> Result<Self> {
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
            options,
            today,
        })
    }

    pub fn next(&self) -> Result<Self> {
        let next_month = (self.month % 12) + 1;
        let year = if next_month > self.month {
            self.year
        } else {
            self.year + 1
        };
        Self::new(next_month, year, self.hm, self.options.clone(), self.today)
    }

    pub fn prev(&self) -> Result<Self> {
        let prev_month = if self.month == 1 { 12 } else { self.month - 1 };
        let year = if prev_month < self.month {
            self.year
        } else {
            self.year - 1
        };
        Self::new(prev_month, year, self.hm, self.options.clone(), self.today)
    }

    pub fn get_matrix(&self) -> Vec<Vec<String>> {
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
                Some((cr, _)) if self.options.highlight_today && cr == self.today => {
                    self.day_string(cr).black().on_white().to_string()
                }
                Some((cr, _)) if weekends.contains(&cr.weekday()) => {
                    self.day_string(cr).green().to_string()
                }
                Some((cr, true)) => self.day_string(cr).red().to_string(),
                Some((cr, false)) => self.day_string(cr),
                None => String::new(),
            })
            .collect::<Vec<_>>()
            .chunks(7)
            .map(|x| x.to_vec())
            .collect()
    }

    fn day_string(&self, date: NaiveDate) -> String {
        if self.options.julian {
            format!("{:3}", date.ordinal())
        } else {
            date.day().to_string()
        }
    }

    pub fn format(&self) -> String {
        const WEEKDAYS: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
        const WEEKDAYS_JULIAN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

        let headers = if self.options.julian {
            &WEEKDAYS_JULIAN
        } else {
            &WEEKDAYS
        };

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
            headers
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

    fn default_options() -> DisplayOptions {
        DisplayOptions {
            highlight_today: true,
            julian: false,
        }
    }

    fn distant_past() -> NaiveDate {
        NaiveDate::from_ymd_opt(1900, 1, 1).expect("valid date")
    }

    #[test]
    fn prev_from_january_wraps_to_december_previous_year() {
        let hm = HashMap::new();
        let dm = DisplayMonth::new(1, 2024, &hm, default_options(), distant_past())
            .expect("valid display month");
        let prev = dm.prev().expect("previous month available");

        assert_eq!(prev.month, 12);
        assert_eq!(prev.year, 2023);
    }

    #[test]
    fn next_from_december_wraps_to_january_next_year() {
        let hm = HashMap::new();
        let dm = DisplayMonth::new(12, 2023, &hm, default_options(), distant_past())
            .expect("valid display month");
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
        let dm = DisplayMonth::new(1, 1970, &hm, default_options(), distant_past())
            .expect("valid display month");

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
        let dm = DisplayMonth::new(1, 2024, &hm, default_options(), distant_past())
            .expect("valid display month");

        let formatted = dm.format();
        assert!(formatted.contains("Mo"));
        assert!(formatted.contains("Su"));
    }

    #[test]
    fn no_highlight_option_disables_today_highlight() {
        let _color_guard = ColorGuard::enable();
        let hm = HashMap::new();
        let today = NaiveDate::from_ymd_opt(1970, 1, 15).expect("valid date");
        let options = DisplayOptions {
            highlight_today: false,
            julian: false,
        };
        let dm = DisplayMonth::new(1, 1970, &hm, options, today).expect("valid display month");

        let matrix = dm.get_matrix();
        let flattened: Vec<&String> = matrix.iter().flat_map(|row| row.iter()).collect();
        // No cell should have the on_white background (black text on white)
        assert!(
            !flattened.iter().any(|cell| cell.contains("\u{1b}[30m")),
            "no today highlight expected when disabled"
        );
    }

    #[test]
    fn julian_option_displays_ordinal_days() {
        let _color_guard = ColorGuard::enable();
        let hm = HashMap::new();
        let options = DisplayOptions {
            highlight_today: true,
            julian: true,
        };
        let dm = DisplayMonth::new(1, 1970, &hm, options, distant_past())
            .expect("valid display month");

        let matrix = dm.get_matrix();
        let flattened: Vec<&String> = matrix.iter().flat_map(|row| row.iter()).collect();
        // January 1 should be day 1, January 31 should be day 31
        assert!(
            flattened.iter().any(|cell| cell.contains("  1")),
            "expected day 1 as ordinal"
        );
        assert!(
            flattened.iter().any(|cell| cell.contains(" 31")),
            "expected day 31 as ordinal"
        );
    }

    #[test]
    fn julian_format_includes_wider_headers() {
        let _color_guard = ColorGuard::enable();
        let hm = HashMap::new();
        let options = DisplayOptions {
            highlight_today: true,
            julian: true,
        };
        let dm = DisplayMonth::new(1, 2024, &hm, options, distant_past())
            .expect("valid display month");

        let formatted = dm.format();
        assert!(formatted.contains("Mon"));
        assert!(formatted.contains("Sun"));
    }
}
