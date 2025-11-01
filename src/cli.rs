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
        let env = actions::RealEnvironment::default();
        match self.action {
            Some(Commands::Delete { day, month }) => actions::delete(&env, day, month),
            Some(Commands::Display { mode: Some(mode) }) => actions::display(&env, mode),
            Some(Commands::Display { mode: None }) | None => actions::display(&env, Mode::Q),
            Some(Commands::Add { day, month }) => actions::add(&env, day, month),
        }
    }
}
