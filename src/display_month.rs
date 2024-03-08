use ascii_table::{Align, AsciiTable};
use chrono::{Datelike, Month, NaiveDate, Weekday};
use colored::Colorize;

use crate::HM;

pub struct DisplayMonth<'a> {
    pub month: u32,
    pub year: i32,
    first_day: NaiveDate,
    last_day: NaiveDate,
    hm: &'a HM,
}

impl<'a> DisplayMonth<'a> {
    pub fn new(month: u32, year: i32, hm: &'a HM) -> Self {
        let first_day = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
        let last_day = NaiveDate::from_ymd_opt(year, month + 1, 1)
            .unwrap_or(NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap())
            .pred_opt()
            .unwrap();
        Self {
            month,
            year,
            first_day,
            last_day,
            hm,
        }
    }
    pub fn next(&self) -> Self {
        let next_month = (self.month % 12) + 1;
        let year = if next_month > self.month {
            self.year
        } else {
            self.year + 1
        };
        Self::new(next_month, year, self.hm)
    }
    pub fn prev(&self) -> Self {
        let prev_month = (self.month % 12 - 1) % 12;
        let year = if prev_month < self.month {
            self.year
        } else {
            self.year - 1
        };
        Self::new(prev_month, year, self.hm)
    }
    pub fn get_matrix(&self) -> Vec<Vec<String>> {
        let mut curr_day = self.first_day;
        let first_index = self.first_day.weekday().number_from_monday();
        (1..self.last_day.day() + first_index)
            .map(|i| {
                if i < first_index {
                    return "".into();
                }

                let cr = curr_day;
                curr_day = curr_day.succ_opt().unwrap();

                let day = cr.day();
                let is_holiday = *self.hm.get(&(day, self.month)).unwrap_or(&false);
                if is_holiday || cr.weekday() == Weekday::Sun || cr.weekday() == Weekday::Sat {
                    return day.to_string().red().to_string();
                } else if cr == chrono::Utc::now().naive_local().date() {
                    return day.to_string().black().on_white().to_string();
                } else {
                    return day.to_string();
                }
            })
            .collect::<Vec<_>>()
            .chunks(7)
            .map(|x| x.to_vec())
            .collect()
    }
    pub fn display(&self) {
        let k = Month::try_from(self.month as u8).unwrap();
        println!("{} {}", k.name(), self.year);
        let mut ascii_table = AsciiTable::default();

        for i in 0..7 {
            let day = Weekday::try_from(i).unwrap();
            ascii_table
                .column(i as usize)
                .set_header(day.to_string()[0..2].to_string())
                .set_align(Align::Center);
        }
        ascii_table.print(self.get_matrix());
    }
}
