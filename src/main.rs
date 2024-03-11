use clap::Parser;
use std::collections::HashMap;

mod cli;
mod display_month;
mod holidays;

type HM = HashMap<(u32, u32), bool>;

fn main() {
    let args = cli::Args::parse();
    args.invoke();
}
