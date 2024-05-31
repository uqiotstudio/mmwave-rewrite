use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Port for server
    #[arg(short, long, default_value_t = 3000)]
    pub port: u16,
}
