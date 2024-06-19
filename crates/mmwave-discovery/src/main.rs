use clap::Parser;
use searchlight::broadcast::{BroadcasterBuilder, ServiceBuilder};
use std::{
    error::Error,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};
use tracing::info;

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
struct Args {
    /// Port for server
    #[arg(short, long, default_value_t = 3000)]
    pub port: u16,

    /// Enable debug logging
    #[arg(short, long, default_value_t = false)]
    pub debug: bool,

    /// Enable relay logging
    #[arg(short, long, default_value_t = false)]
    pub log_relay: bool,

    /// Whether to use tracing
    #[arg(short, long, default_value_t = false)]
    pub tracing: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    if args.tracing {
        mmwave_core::logging::enable_tracing(args.debug, args.log_relay);
    }

    info!("beginning broadcast");
    BroadcasterBuilder::new()
        .loopback()
        .add_service(
            ServiceBuilder::new("_http._tcp.local", "mmwaveserver", args.port)
                .unwrap()
                .add_ip_address(IpAddr::V4(Ipv4Addr::LOCALHOST))
                .add_ip_address(IpAddr::V6(Ipv6Addr::LOCALHOST))
                .build()
                .unwrap(),
        )
        .build(searchlight::net::IpVersion::Both)
        .unwrap()
        .run()?;

    Ok(())
}
