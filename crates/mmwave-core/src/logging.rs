use std::panic;

use indicatif::ProgressStyle;
use tracing::error;
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn enable_tracing(debug: bool) {
    let indicatif_layer =
        IndicatifLayer::new().with_max_progress_bars(100, Some(ProgressStyle::default_bar()));
    let mut filter = EnvFilter::builder()
        .with_default_directive(tracing::Level::INFO.into())
        .from_env()
        .expect("Failed to parse environment filter");

    if debug {
        filter = filter.add_directive("mmwave=debug".parse().expect("Failed to parse directive"));
    }

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(indicatif_layer.get_stderr_writer()))
        .with(indicatif_layer)
        .with(filter)
        .init();

    set_panic_hook();
}

fn set_panic_hook() {
    panic::set_hook(Box::new(|panic_info| {
        error!("Panic occurred: {:?}", panic_info);
    }));
}
