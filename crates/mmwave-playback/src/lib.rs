use async_nats::{
    connection::State,
    jetstream::{
        self,
        kv::{Entry, Store, Watch},
    },
    Client,
};
use async_trait::async_trait;
use chrono::Utc;
use egui::{TextEdit, Ui};
use futures::StreamExt;
use mmwave_core::{
    address::ServerAddress,
    config::Configuration,
    devices::DeviceDescriptor,
    message::{Id, Message, MessageContent, Tag, TagsToSubject},
    nats::get_store,
    pointcloud::PointCloud,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    any::{Any, TypeId},
    error::Error,
    fmt::Display,
    fs::File,
    io::{BufReader, Read},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{select, task::yield_now};
use tracing::{debug, error, info, instrument, warn};

#[derive(PartialEq, Debug, Clone, Serialize, Default)]
pub struct PlaybackDescriptor {
    pub file_path: String, // Path to the file from which data will be read
    pub label_filter: String, // Label filter for playback
}

#[derive(Deserialize)]
struct PlaybackDescriptorHelper {
    file_path: String,
    label_filter: String,
}

impl Eq for PlaybackDescriptor {}

impl std::hash::Hash for PlaybackDescriptor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.file_path.hash(state);
        self.label_filter.hash(state);
    }
}

impl Display for PlaybackDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.file_path, self.label_filter)
    }
}

impl<'de> Deserialize<'de> for PlaybackDescriptor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = PlaybackDescriptorHelper::deserialize(deserializer)?;
        Ok(PlaybackDescriptor {
            file_path: helper.file_path,
            label_filter: helper.label_filter,
        })
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimestampDescriptor {
    pub timestamp: u64, // Timestamp to synchronize playback
}

impl Eq for TimestampDescriptor {}

impl std::hash::Hash for TimestampDescriptor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.timestamp.hash(state);
    }
}

impl Display for TimestampDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.timestamp)
    }
}

#[typetag::serde]
#[async_trait]
impl DeviceDescriptor for PlaybackDescriptor {
    #[instrument(skip_all, fields(self=%self, id=%id))]
    async fn init(self: Box<Self>, id: Id, address: ServerAddress) {
        if let Err(e) = start_playback(*self, id, address).await {
            error!(error=?e, "Playback closed with error");
        }
    }

    fn clone_boxed(&self) -> Box<dyn DeviceDescriptor> {
        Box::new(self.clone())
    }

    fn title(&self) -> String {
        format!("{}", self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("File Path:");
            ui.text_edit_singleline(&mut self.file_path);
        });
        ui.horizontal(|ui| {
            ui.label("Label Filter:");
            ui.text_edit_singleline(&mut self.label_filter);
        });
    }
}

#[typetag::serde]
#[async_trait]
impl DeviceDescriptor for TimestampDescriptor {
    #[instrument(skip_all, fields(self=%self, id=%id))]
    async fn init(self: Box<Self>, id: Id, address: ServerAddress) {
        // Initialization logic for TimestampDescriptor if needed
    }

    fn clone_boxed(&self) -> Box<dyn DeviceDescriptor> {
        Box::new(self.clone())
    }

    fn title(&self) -> String {
        format!("{}", self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("Timestamp:");
            ui.text_edit_singleline(&mut self.timestamp.to_string());
        });
    }
}

#[instrument(skip_all)]
async fn start_playback(
    mut descriptor: PlaybackDescriptor,
    id: Id,
    address: ServerAddress,
) -> Result<(), Box<dyn Error>> {
    // Connect to the NATS server
    let client = async_nats::connect(address.address().to_string()).await?;
    let jetstream = jetstream::new(client.clone());

    // Listen for config updates on a separate task
    let store = get_store(jetstream).await?;
    let mut entries = store.watch("config").await?;

    let mut interval = tokio::time::interval(Duration::from_millis(5000));
    loop {
        // Verify the client connection state
        if client.connection_state() == State::Disconnected {
            return Err(String::from("Lost connection to NATS").into());
        }

        if let Err(e) = run_playback(
            &client,
            descriptor.clone(),
            id,
        )
        .await
        {
            error!(error=%e, "Playback stopped running");
        }
        interval.tick().await;
    }
}

#[instrument(skip_all)]
async fn run_playback(
    client: &Client,
    descriptor: PlaybackDescriptor,
    id: Id,
) -> Result<(), Box<dyn Error>> {
    let file = File::open(&descriptor.file_path)?;
    let mut reader = BufReader::new(file);
    let mut content = String::new();
    reader.read_to_string(&mut content)?;
    
    // Replace all instances of "null" with "0.0"
    let sanitized_content = content.replace("null", "0.0");
    
    let json_array: Vec<PointCloud> = serde_json::from_str(&sanitized_content)?;
    let time_started = Utc::now();
    let mut ptc_time_started = None;

    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
    for pointcloud in json_array {
        if ptc_time_started == None {
            ptc_time_started = Some(pointcloud.time);
        }
        if (pointcloud.labels.iter().any(|label| {label.contains(&descriptor.label_filter)}) && descriptor.label_filter != "") || (pointcloud.labels.len() == 0 && descriptor.label_filter == "") {
            let time_passed = Utc::now() - time_started;
            let ptc_time = pointcloud.time;
            let ptc_time_passed = ptc_time - ptc_time_started.unwrap();

            if time_passed > ptc_time_passed {
                continue;
            }

            let message = Message {
                content: MessageContent::PointCloud(pointcloud),
                tags: vec![Tag::Pointcloud, Tag::FromId(id)],
                timestamp: chrono::Utc::now(),
            };
            let subject = message.tags.clone().to_subject();
            let payload = bincode::serialize(&message)?.into();
            client.publish(subject, payload).await?;
            interval.tick().await;
        }
    }

    Ok(())
}

fn maintain_config(
    entry: Entry,
    descriptor: &mut PlaybackDescriptor,
    id: Id,
) -> Result<(), Box<dyn Error>> {
    let Ok(configuration) = serde_json::from_slice::<Configuration>(&entry.value) else {
        return Ok(());
    };

    for mut device_config in configuration.descriptors {
        if device_config.id != id {
            continue;
        }

        let erased_desc = device_config.device_descriptor.as_any();

        let updated_desc = match erased_desc.downcast_ref::<PlaybackDescriptor>() {
            Some(playback_desc) => playback_desc,
            None => {
                tracing::error!(
                    "Failed to downcast: actual type id = {:?}, expected type id = {:?}",
                    erased_desc.type_id(),
                    TypeId::of::<Box<PlaybackDescriptor>>()
                );
                continue;
            }
        };

        if descriptor.file_path != updated_desc.file_path {
            info!("Updated playback descriptor file path");
            descriptor.file_path = updated_desc.file_path.clone();
        }

        if descriptor.label_filter != updated_desc.label_filter {
            info!("Updated playback descriptor label filter");
            descriptor.label_filter = updated_desc.label_filter.clone();
        }
    }

    Ok(())
}
