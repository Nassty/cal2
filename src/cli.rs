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
    Add { day: u32, month: u32 },
    Display { mode: Option<Mode> },
    Delete { day: u32, month: u32 },
}

impl Args {
    pub fn invoke(&self) {
        match self.action {
            Some(Commands::Delete { day, month }) => actions::delete(day, month),
            Some(Commands::Display { mode: Some(mode) }) => actions::display(mode),
            Some(Commands::Display { mode: None }) | None => actions::display(Mode::Q),
            Some(Commands::Add { day, month }) => actions::add(day, month),
        }
    }
}
