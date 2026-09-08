use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

pub fn initialize_tracing() {
    let stdout_layer = tracing_subscriber::fmt::layer().compact().with_filter(
        EnvFilter::new("info")
            .add_directive("subduction_core=trace".parse().unwrap())
            .add_directive("sedimentree_core=trace".parse().unwrap())
            .add_directive("subduction_redb_storage=trace".parse().unwrap())
            .add_directive("subduction_websocket=trace".parse().unwrap())
            .add_directive("subduction_crypto=trace".parse().unwrap()),
    );

    if let Err(e) = tracing_subscriber::registry().with(stdout_layer).try_init() {
        tracing::error!("Failed to initialize tracing subscriber: {:?}", e);
    } else {
        tracing::info!("Tracing subscriber initialized");
    }
}
