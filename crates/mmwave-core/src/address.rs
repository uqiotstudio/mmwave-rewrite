use reqwest::Url;
use searchlight::{
    discovery::{DiscoveryBuilder, DiscoveryEvent},
    dns::rr::RData,
    net::IpVersion,
};
use std::net::{IpAddr, SocketAddr};
use tracing::{debug, info, info_span, instrument, warn};

#[derive(Clone, Copy, Debug)]
pub struct ServerAddress {
    address: SocketAddr,
    is_fixed: bool,
}

impl ServerAddress {
    pub async fn new(ip: Option<IpAddr>, port: u16) -> Self {
        let (address, is_fixed) = match ip {
            Some(ip) => (SocketAddr::new(ip, port), true),
            None => {
                let address = discover_service().await;
                (address, false)
            }
        };

        Self { address, is_fixed }
    }

    #[instrument(fields(self = %self.address()))]
    pub async fn refresh(&mut self) {
        if !self.is_fixed {
            self.address = discover_service().await;
        }
        debug!(address=?self.address);
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn url(&self) -> Url {
        Url::parse(&format!("http://{}", self.address())).expect("Unable to parse url")
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
                        let _ = found_tx.send(addr);
                    } else {
                        warn!("The server should use ipv4");
                    }
                }
            }
        });

    let server = found_rx.recv().expect("Lost mDNS discovery channel");

    let _ = discovery.shutdown();

    info!(selected = %server);
    return server;
}
