use prettytable::{format, Cell, Row, Table};
use std::collections::HashMap;

mod display_month;
mod holidays;
use chrono::Datelike;
use clap::Parser;
use holidays::{get_filename, get_holidays, load, save};
mod cli;

type HM = HashMap<(u32, u32), bool>;

fn main() {
    let cli::Args { action, day, month } = cli::Args::parse();
    match &*action {
        "display" => {
            let now = chrono::Utc::now();
            let hm = get_holidays(now.year());
            let dm = display_month::DisplayMonth::new(now.month(), now.year(), &hm);
            let mut cals = Vec::new();
            cals.push(dm.prev());
            cals.push(dm.clone());
            cals.push(dm.next());
            let mut table = Table::new();
            let format = format::FormatBuilder::new().padding(0, 0).build();
            table.set_format(format);

            table.add_row(Row::new(
                cals.iter()
                    .map(|x| {
                        let mut c = Cell::new(&x.month_name);
                        c.align(format::Alignment::CENTER);
                        c
                    })
                    .collect(),
            ));
            table.add_row(Row::new(
                cals.iter().map(|x| Cell::new(&x.format())).collect(),
            ));
            table.printstd();
        }
        "add" => match (day, month) {
            (Some(day), Some(month)) => {
                let now = chrono::Utc::now();
                let fname = get_filename(now.year());
                let mut hm = load(&fname).unwrap();
                hm.insert((day, month), true);
                save(&fname, &hm);
                println!("OK");
            }

            _ => {
                eprintln!("missing parameters day and month");
            }
        },
        _ => unreachable!(),
    }
}
