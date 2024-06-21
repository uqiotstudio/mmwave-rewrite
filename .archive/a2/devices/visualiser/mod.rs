use std::collections::HashSet;
use std::sync::Arc;

use crossterm::{
    cursor::MoveTo,
    execute,
    terminal::{Clear, ClearType},
};
use serde::{Deserialize, Serialize};
use std::io::{stdout, Write};
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;
use tracing::{error, info, instrument, Instrument};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use crate::core::pointcloud::PointCloud;
use crate::core::{
    message::{Destination, Id, Message},
    pointcloud::IntoPointCloud,
};

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize, Default)]
pub struct VisualiserDescriptor {
    // Add any additional fields if necessary
}

impl std::hash::Hash for VisualiserDescriptor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Add any additional fields if necessary
        "".hash(state);
    }
}

impl Eq for VisualiserDescriptor {}

#[derive(Debug)]
pub struct Visualiser {
    id: Id,
    inbound: broadcast::Sender<Message>,
    outbound: broadcast::Sender<Message>,
}

impl Visualiser {
    pub fn new(id: Id) -> Self {
        Self {
            id,
            inbound: broadcast::channel(100).0,
            outbound: broadcast::channel(100).0,
        }
    }

    #[instrument(skip_all)]
    pub fn channel(&mut self) -> (broadcast::Sender<Message>, broadcast::Receiver<Message>) {
        (self.inbound.clone(), self.outbound.subscribe())
    }

    #[instrument(skip_all)]
    pub fn start(self) -> JoinHandle<()> {
        let Self {
            inbound,
            outbound,
            id,
        } = self;
        let mut inbound_rx = inbound.subscribe();

        // register data messages to come to this id
        let _ = outbound.send(Message {
            content: crate::core::message::MessageContent::RegisterId(
                HashSet::from([id]),
                HashSet::from([Destination::Visualiser]),
            ),
            destination: HashSet::from([Destination::Server, Destination::Id(id.to_machine())]),
            timestamp: chrono::Utc::now(),
        });

        // Listen for messages. Specifically interested in DataMessages
        tokio::task::spawn(
            {
                async move {
                    let outbound = outbound.clone();
                    let inbound = inbound.clone();
                    while let Ok(message) = inbound_rx.recv().await {
                        match message.content {
                            crate::core::message::MessageContent::DataMessage(data) => {
                                let point_cloud: PointCloud = data.into_point_cloud();
                                plot_point_cloud(&point_cloud);
                            }
                            other => {
                                error!("unsupported message");
                            }
                        }
                    }
                }
            }
            .instrument(tracing::Span::current()),
        )
    }
}

fn plot_point_cloud(point_cloud: &PointCloud) {
    let mut stdout = stdout();
    execute!(stdout, Clear(ClearType::All));

    for point in &point_cloud.points {
        let x = (point[0] * 10.0) as u16;
        let y = (point[1] * 10.0) as u16;
        execute!(stdout, MoveTo(x, y),);
        print!(".");
    }

    stdout.flush();
    // Ok(())
}
