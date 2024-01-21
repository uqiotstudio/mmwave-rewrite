extern crate tui;
use std::time::Duration;

use radars::pointcloud::PointCloud;
use reqwest;
use tokio;
use tui::backend::Backend;
use tui::backend::CrosstermBackend;
use tui::layout::Rect;
use tui::style::{Color, Style};
use tui::symbols;
use tui::terminal::Frame;
use tui::widgets::canvas::Shape;
use tui::widgets::{canvas::Canvas, Block, Borders, Widget};
use tui::Terminal;

struct Point {
    x: f64,
    y: f64,
    c: Color,
}

struct RealTimePlot {
    points: Vec<Point>,
}

impl RealTimePlot {
    fn new() -> RealTimePlot {
        RealTimePlot { points: Vec::new() }
    }

    fn add_point(&mut self, x: f64, y: f64, c: Color) {
        self.points.push(Point { x, y, c });
    }

    fn clear(&mut self) {
        self.points = Vec::new();
    }

    fn draw<B: Backend>(&self, f: &mut Frame<B>, area: Rect) {
        let canvas = Canvas::default()
            .block(Block::default().borders(Borders::ALL).title("Plot"))
            .x_bounds([-10.0, 10.0])
            .y_bounds([-10.0, 10.0])
            .paint(|ctx| {
                for p in &self.points {
                    ctx.draw(&tui::widgets::canvas::Points {
                        coords: &[(p.x, p.y)],
                        color: p.c,
                    });
                    // ctx.print(p.x, p.y, symbols::DOT, Style::default().fg(p.color));
                    // ctx.print(p.x, p.y, "x");
                }
            });

        f.render_widget(canvas, area);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdout = std::io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let url = "http://127.0.0.1:3000/get_pointcloud";

    // Terminal initialization and other setup...

    let mut plot = RealTimePlot::new();

    // Add points to the plot
    plot.add_point(5.0, 5.0, Color::Red);
    plot.add_point(0.0, 0.0, Color::Blue);

    let refresh_rate = std::time::Duration::from_millis(1000 / 60);

    // Draw the plot in the terminal
    loop {
        let cycle_start = std::time::Instant::now();

        match reqwest::get(url).await {
            Ok(resp) => match resp.text().await {
                Ok(text) => {
                    if let Ok(pc) = serde_json::from_str::<PointCloud>(&text) {
                        plot.clear();
                        for (point, meta) in pc.points.iter().zip(pc.metadata) {
                            plot.add_point(
                                point[0] as f64,
                                point[1] as f64,
                                match meta.device.unwrap_or("".to_string()) {
                                    s if s.starts_with("50528259") => Color::Red,
                                    _ => Color::White,
                                },
                            )
                        }
                    }
                }
                Err(e) => eprintln!("Failed to read response text: {}", e),
            },
            Err(e) => eprintln!("Failed to make request: {}", e),
        }

        // Clear the terminal
        terminal.clear()?;

        // Draw the plot in the terminal
        terminal.draw(|f| {
            let size = f.size();
            plot.draw(f, size);
        })?;

        if let Some(sleep_duration) = refresh_rate.checked_sub(cycle_start.elapsed()) {
            std::thread::sleep(sleep_duration);
        }
    }
}
