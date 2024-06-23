use async_nats::{
    connection::State,
    jetstream::{
        self,
        kv::{Entry, Store, Watch},
    },
    Client,
};
use async_trait::async_trait;
use egui::{TextEdit, Ui};
use futures::StreamExt;
use mmwave_core::{
    address::ServerAddress,
    config::Configuration,
    devices::DeviceDescriptor,
    message::{Id, Message, MessageContent, Tag},
    nats::get_store,
    pointcloud::PointCloud,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    any::{Any, TypeId},
    error::Error,
    fmt::Display,
    fs::File,
    io::{self, BufWriter, Write},
    time::Duration,
};
use tokio::{select, task::yield_now};
use tracing::{debug, error, info, instrument, warn};

#[derive(PartialEq, Debug, Clone, Serialize, Default)]
pub struct RecordingDescriptor {
    pub file_path: String, // Path to the file where data will be recorded
}

#[derive(Deserialize)]
struct RecordingDescriptorHelper {
    file_path: String,
}

struct JsonArrayWriter<W: Write> {
    inner: BufWriter<W>,
    is_first: bool,
}

impl<W: Write> JsonArrayWriter<W> {
    fn new(inner: W) -> Self {
        let mut writer = BufWriter::new(inner);
        writer.write_all(b"[").unwrap();
        JsonArrayWriter {
            inner: writer,
            is_first: true,
        }
    }

    fn write_element(&mut self, element: &PointCloud) -> io::Result<()> {
        if !self.is_first {
            self.inner.write_all(b",\n")?;
        }
        self.is_first = false;
        let serialized_element = serde_json::to_string(element).unwrap();
        self.inner.write_all(serialized_element.as_bytes())?;
        self.inner.flush();
        Ok(())
    }
}

impl<W: Write> Drop for JsonArrayWriter<W> {
    fn drop(&mut self) {
        self.inner.write_all(b"]").unwrap();
        self.inner.flush().unwrap();
    }
}

impl Eq for RecordingDescriptor {}

impl std::hash::Hash for RecordingDescriptor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.file_path.hash(state);
    }
}

impl Display for RecordingDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.file_path)
    }
}

impl<'de> Deserialize<'de> for RecordingDescriptor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = RecordingDescriptorHelper::deserialize(deserializer)?;
        Ok(RecordingDescriptor {
            file_path: helper.file_path,
        })
    }
}

#[typetag::serde]
#[async_trait]
impl DeviceDescriptor for RecordingDescriptor {
    #[instrument(skip_all, fields(self=%self, id=%id))]
    async fn init(self: Box<Self>, id: Id, address: ServerAddress) {
        if let Err(e) = start_recording(*self, id, address).await {
            error!(error=?e, "Recording closed with error");
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
    }
}

#[instrument(skip_all)]
async fn start_recording(
    mut descriptor: RecordingDescriptor,
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

        if let Err(e) = run_recording(
            &client,
            &store,
            &mut entries,
            descriptor.clone(),
            id,
            address,
        )
        .await
        {
            error!(error=%e, "Recording stopped running");
        }
        interval.tick().await;
    }
}

#[instrument(skip_all)]
async fn run_recording(
    client: &Client,
    store: &Store,
    entries: &mut Watch,
    mut descriptor: RecordingDescriptor,
    id: Id,
    address: ServerAddress,
) -> Result<(), Box<dyn Error>> {
    let file = File::create(&descriptor.file_path)?;
    let mut writer = JsonArrayWriter::new(file);
    let mut subscription = client.subscribe("Pointcloud.*").await?;

    loop {
        yield_now().await;
        select! {
            Some(config) = entries.next() => {
                if let Err(e) = maintain_config(config?, &mut descriptor, id.clone(), &mut writer) {
                    info!("Restarting recording device with new config");
                    return Ok(());
                }
            }
            Some(message) = subscription.next() => {
                let message: Message = bincode::deserialize(&message.payload)?;
                if let MessageContent::PointCloud(pointcloud) = message.content {
                    writer.write_element(&pointcloud)?;
                }
            }
        }
    }
}

fn maintain_config(
    entry: Entry,
    descriptor: &mut RecordingDescriptor,
    id: Id,
    writer: &mut JsonArrayWriter<File>,
) -> Result<(), Box<dyn Error>> {
    let Ok(configuration) = serde_json::from_slice::<Configuration>(&entry.value) else {
        return Ok(());
    };

    for mut device_config in configuration.descriptors {
        if device_config.id != id {
            continue;
        }

        let erased_desc = device_config.device_descriptor.as_any();

        let updated_desc = match erased_desc.downcast_ref::<RecordingDescriptor>() {
            Some(rec_desc) => rec_desc,
            None => {
                tracing::error!(
                    "Failed to downcast: actual type id = {:?}, expected type id = {:?}",
                    erased_desc.type_id(),
                    TypeId::of::<Box<RecordingDescriptor>>()
                );
                continue;
            }
        };

        if descriptor.file_path != updated_desc.file_path {
            info!("Updated recording descriptor file path");
            descriptor.file_path = updated_desc.file_path.clone();
            // Close the current file and open a new one
            let new_file = File::create(&descriptor.file_path)?;
            *writer = JsonArrayWriter::new(new_file);
        }
    }

    Ok(())
}
