use std::path::PathBuf;

use arbitrum_config::ArbitrumRethConfig;
use arbitrum_node::ArbitrumRethNode;
use clap::Parser;
use eyre::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Arbitrum-Reth: A high-performance L2 node compatible with Arbitrum Nitro
/// Built with Reth SDK for modular and performant blockchain infrastructure
#[derive(Parser, Debug)]
#[command(name = "arbitrum-reth")]
#[command(
    about = "A high-performance, modular Layer 2 node compatible with Arbitrum Nitro, built with Reth SDK"
)]
#[command(version)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Enable sequencer mode
    #[arg(long)]
    sequencer: bool,

    /// Enable validator mode
    #[arg(long)]
    validator: bool,

    /// Data directory for the node
    #[arg(short, long, default_value = "./data")]
    datadir: PathBuf,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Enable metrics server
    #[arg(long)]
    metrics: bool,

    /// Metrics server address
    #[arg(long, default_value = "127.0.0.1:9090")]
    metrics_addr: String,

    /// Enable Reth SDK debug mode
    #[arg(long)]
    reth_debug: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    init_logging(&cli.log_level)?;

    // Load configuration
    let config = load_config(&cli).await?;

    tracing::info!("========================================");
    tracing::info!("Starting Arbitrum-Reth v{}", env!("CARGO_PKG_VERSION"));
    tracing::info!("Built with Reth SDK for high performance");
    tracing::info!("========================================");
    tracing::info!("Configuration loaded from: {:?}", cli.config);
    tracing::info!("Data directory: {:?}", cli.datadir);

    if cli.sequencer {
        tracing::info!("🚀 Running in SEQUENCER mode");
    }
    if cli.validator {
        tracing::info!("🛡️  Running in VALIDATOR mode");
    }
    if cli.reth_debug {
        tracing::info!("🔧 Reth SDK debug mode enabled");
    }

    // Initialize and start the Arbitrum-Reth node with Reth SDK
    let mut node = ArbitrumRethNode::new(config).await?;

    tracing::info!("Starting Arbitrum-Reth node with Reth SDK integration...");
    node.start().await?;

    tracing::info!("✅ Arbitrum-Reth node started successfully!");
    tracing::info!("Press Ctrl+C to shutdown gracefully");

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    tracing::info!("🛑 Shutdown signal received, stopping node...");

    // Graceful shutdown
    node.stop().await?;

    // Wait for the node to finish
    node.wait_for_shutdown().await?;

    tracing::info!("✅ Node stopped successfully");
    tracing::info!("Goodbye! 👋");

    Ok(())
}

fn init_logging(log_level: &str) -> Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .with_level(true)
                .with_ansi(true),
        )
        .with(filter)
        .init();

    Ok(())
}

async fn load_config(cli: &Cli) -> Result<ArbitrumRethConfig> {
    let mut config = if let Some(config_path) = &cli.config {
        tracing::info!("Loading configuration from: {:?}", config_path);
        ArbitrumRethConfig::load_from_file(config_path).await?
    } else {
        tracing::info!("Using default configuration");
        ArbitrumRethConfig::default()
    };

    // Override with CLI arguments
    config.node.datadir = cli.datadir.clone();
    config.node.sequencer_mode = cli.sequencer;
    config.node.validator_mode = cli.validator;

    if cli.metrics {
        config.metrics.enable = true;
        config.metrics.addr = cli.metrics_addr.clone();
        tracing::info!("Metrics enabled at: {}", cli.metrics_addr);
    }

    // Log configuration summary
    tracing::info!("Configuration summary:");
    tracing::info!("  - Data directory: {:?}", config.node.datadir);
    tracing::info!("  - Sequencer mode: {}", config.node.sequencer_mode);
    tracing::info!("  - Validator mode: {}", config.node.validator_mode);
    tracing::info!("  - Metrics enabled: {}", config.metrics.enable);

    Ok(config)
}
