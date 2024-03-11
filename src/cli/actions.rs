use std::iter::zip;

use crate::cli::Mode;
use crate::holidays::{get_filename, get_holidays, load, save};
use chrono::Datelike;
use prettytable::{format, Cell, Row, Table};
pub fn display(mode: Mode) {
    let now = chrono::Utc::now();
    let hm = get_holidays(now.year());
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
    table.printstd();
}
pub fn add(day: u32, month: u32) {
    let now = chrono::Utc::now();
    let fname = get_filename(now.year());
    let mut hm = load(&fname).unwrap();
    hm.insert((day, month), true);
    save(&fname, &hm);
    println!("OK");
}
