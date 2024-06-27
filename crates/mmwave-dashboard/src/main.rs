mod configuration;

use async_nats::jetstream::kv::{Store, Watch};
use async_nats::{jetstream, Client};
use bincode;
use clap::Parser;
use configuration::ConfigWidget;
use eframe::egui;
use egui::{Color32, Context, Stroke, Vec2b, ViewportBuilder};
use egui_plot::{
    AxisHints, CoordinatesFormatter, HPlacement, Line, PlotItem, PlotPoint, PlotPoints, Points,
    Polygon, Text, VPlacement,
};
use futures::StreamExt;
use mmwave_awr::AwrDescriptor;
use mmwave_core::config::Configuration;
use mmwave_core::logging::enable_tracing;
use mmwave_core::message::{Id, Tag};
use mmwave_core::nats::get_store;
use mmwave_core::point::Point;
use mmwave_core::pointcloud::PointCloud;
use mmwave_core::transform::Transform;
use mmwave_core::{
    address::ServerAddress,
    message::{Message, MessageContent},
};
use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// IP address for server (ipv4)
    #[arg(short, long)]
    pub ip: Option<IpAddr>,

    /// Port for server
    #[arg(short, long, default_value_t = 3000)]
    pub port: u16,

    /// Enable debug logging
    #[arg(short, long, default_value_t = false)]
    pub debug: bool,

    /// Whether to use tracing
    #[arg(short, long, default_value_t = false)]
    pub tracing: bool,
}

struct MyApp {
    ptc_rx: mpsc::Receiver<(Vec<Tag>, Vec<Point>)>,
    cfg_in_rx: mpsc::Receiver<Configuration>,
    cfg_out_tx: mpsc::Sender<Configuration>,
    pointcloud: HashMap<Id, (Instant, Vec<Point>)>,
    config_widget: ConfigWidget,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse arguments
    let args = Args::parse();
    let address = ServerAddress::new(args.ip, args.port).await;

    if args.tracing {
        enable_tracing(args.debug);
    }

    let client = async_nats::connect(address.address().to_string()).await?;
    let jetstream = jetstream::new(client.clone());

    // Listen for config updates on a seperate task
    let store = get_store(jetstream).await?;
    let entries = store.watch("config").await?;

    let _ = eframe::run_native(
        "Pointcloud Listener",
        eframe::NativeOptions {
            viewport: ViewportBuilder::default().with_title("mmwave-dashboard"),
            ..Default::default()
        },
        Box::new(|cc| {
            // Listen for pointclouds and forward them to rx
            let frame = cc.egui_ctx.clone();
            let (ptc_tx, ptc_rx) = mpsc::channel(100);
            tokio::spawn(async move {
                listen_for_pointcloud(frame, client, ptc_tx).await.unwrap();
            });

            // listen for configs and forward them
            let frame = cc.egui_ctx.clone();
            let (cfg_in_tx, cfg_in_rx) = mpsc::channel(10);
            tokio::spawn({
                let store = store.clone();
                async move {
                    listen_for_config(frame, store, entries, cfg_in_tx).await;
                }
            });

            // Send updated configs
            let frame = cc.egui_ctx.clone();
            let (cfg_out_tx, cfg_out_rx) = mpsc::channel(10);
            tokio::spawn(async move {
                update_config(frame, store, cfg_out_rx).await;
            });

            Box::new(MyApp {
                pointcloud: HashMap::new(),
                config_widget: ConfigWidget::default(),
                ptc_rx,
                cfg_in_rx,
                cfg_out_tx,
            })
        }),
    );

    Ok(())
}

async fn listen_for_config(
    frame: Context,
    mut store: Store,
    mut entries: Watch,
    tx: mpsc::Sender<Configuration>,
) {
    if let Ok(Some(config)) = store.get("config").await {
        info!("Found initial config");
        debug!(config=?config, "Initial config");
        match serde_json::from_slice(&config) {
            Ok(config) => {
                let _ = tx.send(config).await;
            }
            Err(e) => {
                error!(error=?e, "Failed to parse config");
                debug!(config=?config, "The incorrect config");
            }
        };
    }

    while let Some(config) = entries.next().await {
        let config: Configuration = match config {
            Err(e) => {
                error!(error=%e, "something went wrong watching for config updates");
                continue;
            }
            Ok(entry) => {
                info!("New config inbound");
                debug!(entry=?entry, "Inbound config");
                match serde_json::from_slice(&entry.value) {
                    Ok(config) => {
                        frame.request_repaint();
                        config
                    }
                    Err(e) => {
                        error!(error=?e, "Failed to parse config");
                        debug!(entry=?entry, "The incorrect entry");
                        continue;
                    }
                }
            }
        };

        if let Err(e) = tx.send(config).await {
            error!(error=?e, "unexpectedly lost in_cfg_tx");
            panic!("This should be impossible");
        }
    }
}

async fn update_config(frame: Context, store: Store, mut rx: mpsc::Receiver<Configuration>) {
    while let Some(config) = rx.recv().await {
        let Ok(serialized) = serde_json::to_string(&config) else {
            error!("Failed to serialize config");
            continue;
        };
        if let Err(e) = store.put("config", serialized.clone().into()).await {
            error!(error=?e,"Failed to put config in store");
            continue;
        }
    }
}

async fn listen_for_pointcloud(
    frame: Context,
    client: Client,
    tx: mpsc::Sender<(Vec<Tag>, Vec<Point>)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut subscription = client.subscribe("Pointcloud.*").await?;

    while let Some(message) = subscription.next().await {
        let message: Message = bincode::deserialize(&message.payload)?;
        if let MessageContent::PointCloud(pointcloud) = message.content {
            let _ = tx
                .send((
                    message.tags,
                    pointcloud.points.iter().map(|&p| p.into()).collect(),
                ))
                .await;
            frame.request_repaint();
        }
    }

    Ok(())
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Ok((tags, new_points)) = self.ptc_rx.try_recv() {
            for tag in tags {
                let Tag::FromId(id) = tag else {
                    continue;
                };

                self.pointcloud.insert(id, (Instant::now(), new_points));
                break;
            }
        };

        if let Ok(config) = self.cfg_in_rx.try_recv() {
            self.config_widget.inbound_config = Some(config);
        }

        if let Some(config) = &self.config_widget.outbound_config {
            let _ = self.cfg_out_tx.try_send(config.clone());
            self.config_widget.outbound_config = None;
        }

        self.pointcloud
            .retain(|k, (t, p)| t.elapsed() < Duration::from_millis(1000));

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::SidePanel::right("right_panel")
                .resizable(true)
                .default_width(200.0)
                .width_range(100.0..=800.0)
                .show_inside(ui, |ui| self.config_widget.ui(ui));

            egui::CentralPanel::default().show_inside(ui, |ui| {
                egui_plot::Plot::new("pointcloud_plot")
                    .allow_zoom(true)
                    .allow_drag(true)
                    .auto_bounds(Vec2b::new(false, false))
                    .show_grid(true)
                    .data_aspect(1.0)
                    .coordinates_formatter(
                        egui_plot::Corner::LeftBottom,
                        CoordinatesFormatter::default(),
                    )
                    .show(ui, |plot_ui| {
                        for (id, (time, pointcloud)) in &self.pointcloud {
                            let (old_transform, new_transform) = {
                                (
                                    if let Some(cfg) = self
                                        .config_widget
                                        .config_original
                                        .descriptors
                                        .iter()
                                        .find(|&d| d.id == *id)
                                    {
                                        cfg.device_descriptor.transform().unwrap_or_default()
                                    } else {
                                        Transform::default()
                                    },
                                    if let Some(cfg) = self
                                        .config_widget
                                        .config
                                        .descriptors
                                        .iter()
                                        .find(|&d| d.id == *id)
                                    {
                                        cfg.device_descriptor.transform().unwrap_or_default()
                                    } else {
                                        Transform::default()
                                    },
                                )
                            };
                            let points = PlotPoints::Owned(
                                pointcloud
                                    .iter()
                                    .map(|&p| {
                                        let p =
                                            new_transform.apply(old_transform.unapply(p.into()));
                                        PlotPoint {
                                            x: p[0] as f64,
                                            y: p[1] as f64,
                                        }
                                    })
                                    .collect(),
                            );
                            let rgb = self
                                .config_widget
                                .colors
                                .entry(*id)
                                .or_insert([1.0, 1.0, 1.0])
                                .clone();
                            let points = Points::new(points)
                                .radius(3.0)
                                .color(Color32::from_rgb(
                                    (rgb[0] * 255.0) as u8,
                                    (rgb[1] * 255.0) as u8,
                                    (rgb[2] * 255.0) as u8,
                                ))
                                .filled(false);
                            plot_ui.points(points);
                        }

                        for cfg in self.config_widget.config.descriptors.iter() {
                            if let Some(transform) = cfg.device_descriptor.transform() {
                                let rgb = self
                                    .config_widget
                                    .colors
                                    .entry(cfg.id)
                                    .or_insert([1.0, 1.0, 1.0])
                                    .clone();

                                let color = Color32::from_rgb(
                                    (rgb[0] * 255.0) as u8,
                                    (rgb[1] * 255.0) as u8,
                                    (rgb[2] * 255.0) as u8,
                                );

                                let polygons = vec![
                                    vec![[-1.0, 1.0, -1.0], [1.0, 1.0, -1.0], [0.0, 0.0, 0.0]],
                                    vec![[-1.0, 1.0, 1.0], [1.0, 1.0, 1.0], [0.0, 0.0, 0.0]],
                                    vec![
                                        [-1.0, 1.0, 1.0],
                                        [1.0, 1.0, 1.0],
                                        [1.0, 1.0, -1.0],
                                        [-1.0, 1.0, -1.0],
                                    ],
                                    vec![[-1.0, 1.0, 1.0], [-1.0, 1.0, -1.0], [0.0, 0.0, 0.0]],
                                    vec![[1.0, 1.0, 1.0], [1.0, 1.0, -1.0], [0.0, 0.0, 0.0]],
                                ];

                                for mut polygon in polygons {
                                    plot_ui.polygon(
                                        Polygon::new(PlotPoints::Owned(
                                            polygon
                                                .iter_mut()
                                                .map(|p| {
                                                    p[0] *= 0.05;
                                                    p[1] *= 0.05;
                                                    p[2] *= 0.05;

                                                    let p = transform.apply(*p);
                                                    PlotPoint {
                                                        x: p[0] as f64,
                                                        y: p[1] as f64,
                                                    }
                                                })
                                                .collect(),
                                        ))
                                        .stroke(Stroke {
                                            color,
                                            ..Default::default()
                                        }),
                                    );
                                }

                                let origin = transform.apply([0.0, 0.0, 0.0]);
                                plot_ui.text(
                                    Text::new(
                                        PlotPoint {
                                            x: origin[0] as f64,
                                            y: origin[1] as f64,
                                        },
                                        cfg.title(),
                                    )
                                    .color(color),
                                );
                            }
                        }
                    });
            });
        });

        ctx.request_repaint();
    }
}
