use futures_util::TryFutureExt;
use mmwave::{
    core::config::Configuration,
    sensors::{SensorConfig, SensorDescriptor},
};
use serde_json;
use std::{
    collections::{HashMap, HashSet},
    env,
    time::Duration,
};
use tokio::select;

use reqwest::Url;

struct SensorClient {
    descriptor: SensorDescriptor,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    let ip_address = args.get(1).cloned().unwrap_or("localhost".to_owned());
    let machine_id: usize = args
        .get(2)
        .cloned()
        .unwrap_or("0".to_owned())
        .parse()
        .expect("Requires Positive Integer for machine_id");

    let url = Url::parse(&format!("ws://{}:3000", ip_address)).expect("Unable to parse url");

    let ws_url = Url::parse(&format!("{}/ws", url)).expect("Unable to parse websocket url");
    let get_config_url =
        Url::parse(&format!("{}/get_config", url)).expect("Unable to parse get_config url");
    let set_config_url =
        Url::parse(&format!("{}/set_config", url)).expect("Unable to parse sent_config url");

    let sensors = HashMap::<SensorDescriptor, SensorClient>::new();

    // Periodically check for config updates
    // Filter out sensors with different machine id. Of those that remain:
    // - If a sensor is no longer listed, kill it
    // - If a sensor is added, spawn it
    // - If a sensors descriptor (not transform) is changed, reboot it
    let mut t1 = tokio::task::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(1000));

        loop {
            interval.tick().await;

            // Get a response from the url
            let Ok(resp) = reqwest::get(get_config_url.clone()).await else {
                continue;
            };

            // Convert it into text
            let Ok(text) = resp.text().await else {
                continue;
            };

            // Parse a config
            let Ok(mut updated_config) = serde_json::from_str::<Configuration>(&text) else {
                eprintln!("Unable to parse config from server response");
                continue;
            };

            // We have a valid configuration. Filter it by our machine id
            updated_config
                .descriptors
                .retain(|cfg| cfg.machine_id == machine_id);

            let updated_sensors = updated_config.descriptors;
            let (updated_sensors_desc, updated_sensors_trans): (Vec<_>, Vec<_>) = updated_sensors
                .into_iter()
                .map(|sensor| (sensor.sensor_descriptor, sensor.transform))
                .unzip();

            // Anything in this set is flagged for removal at the end of loop
            let mut removal_flags: HashSet<&SensorDescriptor> = sensors.keys().collect();

            // Go through all descriptors and check for updates/removals
            // as we do this we pop the matched sensors, so that afterwards
            // only the new sensors are left and can be easily added
            for desc in sensors.keys() {
                if updated_sensors_desc.contains(desc) {
                    let index = updated_sensors_desc.iter().position(|n| n == desc).unwrap();
                    removal_flags.remove(desc);

                    // Update the transform to match the new version
                    let sensor_client = sensors.get_mut(&desc);

                    // Our descriptor remains, if there is any change we kill and reinitialize it
                }
            }
        }
    });
}
