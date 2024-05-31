use std::net::SocketAddr;

use reqwest::Url;
use searchlight::{
    discovery::{DiscoveryBuilder, DiscoveryEvent},
    dns::rr::RData,
    net::IpVersion,
};
use tracing::{info, info_span, instrument, warn};

use crate::args::Args;

#[derive(Clone, Copy, Debug)]
pub struct ServerAddress {
    address: SocketAddr,
    is_fixed: bool,
}

impl ServerAddress {
    pub async fn new(args: Args) -> Self {
        let Args {
            ip,
            port,
            machine_id: _,
        } = args;

        let (address, is_fixed) = match ip {
            Some(ip) => (SocketAddr::new(ip, port), true),
            None => {
                let address = discover_service().await;
                (address, false)
            }
        };

        Self { address, is_fixed }
    }

    pub async fn refresh(&mut self) {
        if self.is_fixed {
            println!("Address is fixed, no action taken.");
        } else {
            println!("Attempting to locate a service on the local network.");
            self.address = discover_service().await;
        };
        dbg!(self.address);
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn url(&self) -> Url {
        Url::parse(&format!("http://{}", self.address())).expect("Unable to parse url")
    }

    pub fn url_get_config(&self) -> Url {
        Url::parse(&format!("{}get_config", self.url())).expect("Unable to parse get_config url")
    }

    pub fn url_set_config(&self) -> Url {
        Url::parse(&format!("{}set_config", self.url())).expect("Unable to parse sent_config url")
    }

    pub fn url_ws(&self) -> Url {
        Url::parse(&format!("ws:///{}/ws", self.address())).expect("Unable to parse websocket url")
    }
}

#[instrument(name = "service_discovery")]
async fn discover_service() -> SocketAddr {
    let (found_tx, found_rx) = std::sync::mpsc::sync_channel(10);
    let discovery = DiscoveryBuilder::new()
        .loopback()
        .service("_http._tcp.local")
        .unwrap()
        .build(IpVersion::Both)
        .unwrap()
        .run_in_background(move |event| {
            if let DiscoveryEvent::ResponderFound(responder) = event {
                let (name, port) = responder
                    .last_response
                    .additionals()
                    .iter()
                    .find_map(|record| {
                        if let Some(RData::SRV(srv)) = record.data() {
                            let name = record.name().to_utf8();
                            let port = srv.port();
                            Some((name.to_string(), port))
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| ("Unknown".into(), 0));

                if name.contains("mmwaveserver") {
                    let span = info_span!("service_discovery", record = %name);
                    let _guard = span.enter();

                    info!(ip = %responder.addr.ip());
                    info!(port = port);

                    if responder.addr.is_ipv4() {
                        // Ipv6 Has issues setting up http request urls
                        let mut addr = responder.addr;
                        addr.set_port(port);
                        found_tx.send(addr);
                        return;
                    } else {
                        warn!("The server does not use ipv4");
                    }
                }
            }
        });

    let server = found_rx.recv().expect("Lost mDNS discovery channel");

    discovery.shutdown();

    info!(selected = %server);
    return server;
}
