use clap::Parser;

#[derive(Parser, Debug)] // requires `derive` feature
#[command(term_width = 0)] // Just to make testing across clap features easier
pub struct Args {
    #[arg(
        short,
        default_value_t = String::from("display"),
        value_parser = clap::builder::PossibleValuesParser::new(["display", "add"]),
    )]
    pub action: String,
    #[arg(short, default_value = None)]
    pub month: Option<u32>,
    #[arg(short, default_value = None)]
    pub day: Option<u32>,
}
