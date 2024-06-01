use std::net::IpAddr;

use clap::Parser;
use mmwave::core::message::Id;

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// IP address for server (ipv4)
    #[arg(short, long)]
    pub ip: Option<IpAddr>,

    /// Port for server
    #[arg(short, long, default_value_t = 3000)]
    pub port: u16,

    /// Number of times to greet
    #[arg(short, long)]
    pub machine_id: Id,
}
