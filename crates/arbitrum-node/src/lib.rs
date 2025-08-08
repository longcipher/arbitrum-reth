#![allow(dead_code)]

pub mod reth_integration;

use std::sync::Arc;

use arbitrum_batch_submitter::BatchSubmitter;
use arbitrum_config::ArbitrumRethConfig;
use arbitrum_consensus::ArbitrumConsensus;
use arbitrum_inbox_tracker::InboxTracker;
use arbitrum_pool::ArbitrumTransactionPool;
use arbitrum_storage::ArbitrumStorage;
use arbitrum_validator::Validator;
use eyre::Result;
use reth_chainspec::MAINNET;
use reth_integration::RethNodeHandle;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// The main Arbitrum-Reth node built with Reth SDK
///
/// This integrates Arbitrum-specific components with Reth's modular architecture
pub struct ArbitrumRethNode {
    config: ArbitrumRethConfig,
    // Arbitrum-specific components
    consensus: Arc<ArbitrumConsensus>,
    tx_pool: Arc<ArbitrumTransactionPool>,
    storage: Arc<ArbitrumStorage>,
    batch_submitter: Option<Arc<BatchSubmitter>>,
    inbox_tracker: Option<Arc<InboxTracker>>,
    validator: Option<Arc<Validator>>,
    is_running: Arc<RwLock<bool>>,
    // Reth node handle (placeholder until full integration)
    reth_handle: Option<RethNodeHandle>,
}

impl ArbitrumRethNode {
    /// Create a new Arbitrum-Reth node using the Reth SDK
    pub async fn new(config: ArbitrumRethConfig) -> Result<Self> {
        info!("Initializing Arbitrum-Reth node with Reth SDK");

        // Validate configuration
        config.validate()?;

        // Create data directories
        tokio::fs::create_dir_all(config.db_path()).await?;
        tokio::fs::create_dir_all(config.static_files_path()).await?;

        // Initialize storage layer
        let storage = Arc::new(ArbitrumStorage::new(&config).await?);
        info!("Storage layer initialized");

        // Initialize consensus engine
        let consensus = Arc::new(ArbitrumConsensus::new(&config, storage.clone()).await?);
        info!("Arbitrum consensus engine initialized");

        // Initialize transaction pool
        let tx_pool = Arc::new(ArbitrumTransactionPool::new(&config).await?);
        info!("Arbitrum transaction pool initialized");

        // Initialize batch submitter if sequencer mode is enabled
        let batch_submitter = if config.sequencer.enabled {
            let submitter = Arc::new(BatchSubmitter::new(&config, Arc::clone(&storage)).await?);
            info!("Batch submitter initialized");
            Some(submitter)
        } else {
            None
        };

        // Initialize inbox tracker
        let inbox_tracker = {
            let tracker = Arc::new(InboxTracker::new(&config, Arc::clone(&storage)).await?);
            info!("Inbox tracker initialized");
            Some(tracker)
        };

        // Initialize validator if validator mode is enabled
        let validator = if config.validator.enable {
            let val = Arc::new(Validator::new(&config, Arc::clone(&storage)).await?);
            info!("Validator initialized");
            Some(val)
        } else {
            None
        };

        Ok(Self {
            config,
            consensus,
            tx_pool,
            storage,
            batch_submitter,
            inbox_tracker,
            validator,
            is_running: Arc::new(RwLock::new(false)),
            reth_handle: None,
        })
    }

    /// Build and start the Reth node with Arbitrum-specific customizations
    pub async fn start(&mut self) -> Result<()> {
        let mut running = self.is_running.write().await;
        if *running {
            warn!("Node is already running");
            return Ok(());
        }

        info!("Starting Arbitrum-Reth node with Reth SDK...");

        // For now, use a simplified approach until full Reth SDK integration
        // This demonstrates the intended architecture
        info!("Creating Arbitrum node configuration...");
        let _chain_spec = MAINNET.clone();

        // Launch minimal Reth node integration (scaffold)
        let handle =
            crate::reth_integration::launch_reth_node(&self.config, Some(self.storage.clone()))
                .await?;
        self.reth_handle = Some(handle);
        info!("Reth node launched (scaffold mode)");

        // Start Arbitrum-specific components
        self.start_arbitrum_components().await?;

        *running = true;

        info!("Arbitrum-Reth node started successfully with Reth SDK architecture");
        Ok(())
    }

    /// Start Arbitrum-specific components
    async fn start_arbitrum_components(&self) -> Result<()> {
        // Start storage layer
        self.storage.start().await?;
        info!("Arbitrum storage layer started");

        // Start consensus engine
        self.consensus.start().await?;
        info!("Arbitrum consensus engine started");

        // Start transaction pool
        self.tx_pool.start().await?;
        info!("Arbitrum transaction pool started");

        // Start inbox tracker
        if let Some(ref inbox_tracker) = self.inbox_tracker {
            inbox_tracker.start().await?;
            info!("Arbitrum inbox tracker started");
        }

        // Start batch submitter if enabled
        if let Some(ref batch_submitter) = self.batch_submitter {
            batch_submitter.start().await?;
            info!("Arbitrum batch submitter started");
        }

        // Start validator if enabled
        if let Some(ref validator) = self.validator {
            validator.start().await?;
            info!("Arbitrum validator started");
        }

        // Start metrics server if enabled
        if self.config.metrics.enable {
            self.start_metrics_server().await?;
            info!("Metrics server started on {}", self.config.metrics.addr);
        }

        Ok(())
    }

    /// Create a basic configuration for Arbitrum
    async fn create_arbitrum_node_config(&self) -> Result<()> {
        info!("Creating Arbitrum node configuration...");

        // Use mainnet chain spec for reference, future: create Arbitrum-specific chain spec
        let _chain_spec = MAINNET.clone();

        // TODO: Create proper NodeConfig for Arbitrum
        // This would include:
        // - Chain specification for Arbitrum
        // - Network configuration (peers, ports, etc.)
        // - Database paths and settings
        // - RPC configuration
        // - Metrics and logging setup

        info!("Arbitrum node configuration created");
        Ok(())
    }

    /// Start metrics server
    async fn start_metrics_server(&self) -> Result<()> {
        info!("Starting metrics server on {}", self.config.metrics.addr);
        // TODO: Implement actual metrics server
        // For now, just log that it would start
        Ok(())
    }

    /// Stop the node
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.is_running.write().await;
        if !*running {
            warn!("Node is already stopped");
            return Ok(());
        }

        info!("Stopping Arbitrum-Reth node...");

        // Stop Arbitrum-specific components first
        self.stop_arbitrum_components().await?;

        // Stop the Reth node
        if let Some(handle) = &self.reth_handle {
            handle.stop().await?;
            info!("Reth node stopped (scaffold mode)");
        } else {
            info!("Reth node handle not present; nothing to stop");
        }

        *running = false;
        info!("Arbitrum-Reth node stopped successfully");
        Ok(())
    }

    /// Stop Arbitrum-specific components
    async fn stop_arbitrum_components(&self) -> Result<()> {
        // Stop validator if running
        if let Some(ref validator) = self.validator {
            validator.stop().await?;
            info!("Arbitrum validator stopped");
        }

        // Stop batch submitter if running
        if let Some(ref batch_submitter) = self.batch_submitter {
            batch_submitter.stop().await?;
            info!("Arbitrum batch submitter stopped");
        }

        // Stop inbox tracker if running
        if let Some(ref inbox_tracker) = self.inbox_tracker {
            inbox_tracker.stop().await?;
            info!("Arbitrum inbox tracker stopped");
        }

        // Stop transaction pool
        self.tx_pool.stop().await?;
        info!("Arbitrum transaction pool stopped");

        // Stop consensus engine
        self.consensus.stop().await?;
        info!("Arbitrum consensus engine stopped");

        // Stop storage layer
        self.storage.stop().await?;
        info!("Arbitrum storage layer stopped");

        Ok(())
    }

    /// Wait for the node to finish
    pub async fn wait_for_shutdown(&self) -> Result<()> {
        if let Some(handle) = &self.reth_handle {
            handle.wait().await?;
            info!("Reth node shutdown complete (scaffold mode)");
        } else {
            info!("No Reth handle; nothing to wait for");
        }
        Ok(())
    }

    /// Check if the node is running
    pub async fn is_running(&self) -> bool {
        *self.is_running.read().await
    }

    /// Get node configuration
    pub fn config(&self) -> &ArbitrumRethConfig {
        &self.config
    }

    /// Get access to the Reth node handle (scaffold)
    pub fn reth_handle(&self) -> Option<&RethNodeHandle> {
        self.reth_handle.as_ref()
    }

    /// Get current sync status
    pub async fn sync_status(&self) -> SyncStatus {
        // Get sync status from Arbitrum components
        let (current_block, highest_block, blocks_behind) =
            if let Some(ref tracker) = self.inbox_tracker {
                let stats = tracker.get_stats().await;
                (
                    stats.last_processed_l1_block,
                    stats.latest_l1_block,
                    stats.blocks_behind,
                )
            } else {
                (0, 0, 0)
            };

        let is_syncing = blocks_behind > 0;
        let sync_progress = if highest_block > 0 {
            current_block as f64 / highest_block as f64
        } else {
            1.0
        };

        SyncStatus {
            is_syncing,
            current_block,
            highest_block,
            sync_progress,
        }
    }

    /// Get node health status
    pub async fn health_status(&self) -> HealthStatus {
        let mut errors = Vec::new();
        let is_running = self.is_running().await;

        // Check component health
        if !is_running {
            errors.push("Node is not running".to_string());
        }

        // Check if Reth node is healthy
        // Future: Check actual Reth node handle
        // if self.reth_handle().is_none() {
        //     errors.push("Reth node handle is not available".to_string());
        // }

        // TODO: Add more health checks
        // - Check if components are responding
        // - Check database connectivity
        // - Check L1 connectivity
        // - Check memory usage

        HealthStatus {
            is_healthy: errors.is_empty() && is_running,
            peer_count: 0, // TODO: Get actual peer count from Reth networking
            last_block_time: chrono::Utc::now(),
            errors,
        }
    }

    /// Get comprehensive node statistics
    pub async fn get_node_stats(&self) -> NodeStats {
        let sync_status = self.sync_status().await;
        let health_status = self.health_status().await;

        // Get component statistics
        let tx_pool_stats = self.tx_pool.get_stats().await;
        let storage_stats = self.storage.get_stats().await;

        let batch_submitter_stats = if let Some(ref submitter) = self.batch_submitter {
            Some(submitter.get_stats().await)
        } else {
            None
        };

        let inbox_tracker_stats = if let Some(ref tracker) = self.inbox_tracker {
            Some(tracker.get_stats().await)
        } else {
            None
        };

        let validator_stats = if let Some(ref validator) = self.validator {
            Some(validator.get_stats().await)
        } else {
            None
        };

        NodeStats {
            sync_status,
            health_status,
            tx_pool_stats,
            storage_stats,
            batch_submitter_stats,
            inbox_tracker_stats,
            validator_stats,
        }
    }
}

/// Comprehensive node statistics
#[derive(Debug, Clone)]
pub struct NodeStats {
    pub sync_status: SyncStatus,
    pub health_status: HealthStatus,
    pub tx_pool_stats: arbitrum_pool::PoolStats,
    pub storage_stats: arbitrum_storage::StorageStats,
    pub batch_submitter_stats: Option<arbitrum_batch_submitter::BatchSubmitterStats>,
    pub inbox_tracker_stats: Option<arbitrum_inbox_tracker::InboxTrackerStats>,
    pub validator_stats: Option<arbitrum_validator::ValidatorStats>,
}

/// Sync status information
#[derive(Debug, Clone)]
pub struct SyncStatus {
    pub is_syncing: bool,
    pub current_block: u64,
    pub highest_block: u64,
    pub sync_progress: f64,
}

/// Health status information
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub is_healthy: bool,
    pub peer_count: usize,
    pub last_block_time: chrono::DateTime<chrono::Utc>,
    pub errors: Vec<String>,
}
