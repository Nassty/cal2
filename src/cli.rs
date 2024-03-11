mod actions;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Args {
    #[command(subcommand)]
    pub action: Option<Commands>,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum Mode {
    Q,
    Month,
    Year,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Add { month: u32, day: u32 },
    Display { mode: Option<Mode> },
}

impl Args {
    pub fn invoke(&self) {
        match self.action {
            Some(Commands::Display { mode: Some(mode) }) => actions::display(mode),
            Some(Commands::Display { mode: None }) | None => actions::display(Mode::Month),
            Some(Commands::Add { day, month }) => actions::add(day, month),
        }
    }
}
