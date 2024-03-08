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
            dm.prev().display();
            dm.display();
            dm.next().display();
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
