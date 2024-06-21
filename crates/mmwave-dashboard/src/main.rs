use async_nats::Client;
use bincode;
use clap::Parser;
use eframe::egui;
use futures::StreamExt;
use mmwave_core::pointcloud::PointCloud;
use mmwave_core::{
    address::ServerAddress,
    message::{Message, MessageContent},
};
use tokio::sync::mpsc;
use tracing::info;

#[derive(Parser)]
struct Args {
    #[arg(short, long, default_value = "4222")]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse arguments
    let args = Args::parse();
    let address = ServerAddress::new(None, args.port).await;

    let client = async_nats::connect(address.address().to_string()).await?;
    let (tx, rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        listen_for_pointcloud(client, tx).await.unwrap();
    });

    eframe::run_native(
        "Pointcloud Listener",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Box::new(MyApp::new(rx))),
    );

    Ok(())
}

async fn listen_for_pointcloud(
    client: Client,
    tx: mpsc::UnboundedSender<(usize, Vec<[f32; 4]>)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut subscription = client.subscribe("pointcloud.awr").await?;
    let mut count = 0;

    while let Some(message) = subscription.next().await {
        count += 1;
        info!("Received pointcloud message: {:?}", message);

        let message: Message = bincode::deserialize(&message.payload)?;
        if let MessageContent::PointCloud(pointcloud) = message.content {
            tx.send((count, pointcloud.points)).unwrap();
        }
    }

    Ok(())
}

struct MyApp {
    rx: mpsc::UnboundedReceiver<(usize, Vec<[f32; 4]>)>,
    count: usize,
    points: Vec<[f32; 4]>,
}

impl MyApp {
    fn new(rx: mpsc::UnboundedReceiver<(usize, Vec<[f32; 4]>)>) -> Self {
        Self {
            rx,
            count: 0,
            points: Vec::new(),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Ok((count, new_points)) = self.rx.try_recv() {
            self.count = count;
            self.points = (new_points);
        };

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Pointcloud Listener");
            ui.label(format!("Messages received: {}", self.count));

            let (response, painter) =
                ui.allocate_painter(ui.available_size(), egui::Sense::hover());
            let canvas_center = response.rect.center();

            for point in &self.points {
                let pos = canvas_center + egui::Vec2::new(point[0] * 100.0, point[1] * 100.0);
                painter.circle_filled(pos, 2.0, egui::Color32::WHITE);
            }
        });

        ctx.request_repaint();
    }
}
