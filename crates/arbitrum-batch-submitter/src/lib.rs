use std::{sync::Arc, time::Duration};

use alloy_primitives::B256;
use arbitrum_config::ArbitrumRethConfig;
use arbitrum_storage::{ArbitrumBatch, ArbitrumBlock, ArbitrumStorage};
use eyre::Result;
use tokio::{sync::RwLock, time::interval};
use tracing::{debug, error, info, warn};

/// Batch submitter responsible for submitting L2 batches to L1
pub struct BatchSubmitter {
    config: ArbitrumRethConfig,
    storage: Arc<ArbitrumStorage>,
    is_running: Arc<RwLock<bool>>,
    last_submitted_block: Arc<RwLock<u64>>,
    // TODO: Add L1 client for submitting batches
    // l1_client: Arc<dyn L1Client>,
}

#[allow(dead_code)]
impl BatchSubmitter {
    /// Create a new batch submitter
    pub async fn new(config: &ArbitrumRethConfig, storage: Arc<ArbitrumStorage>) -> Result<Self> {
        info!("Initializing batch submitter");

        Ok(Self {
            config: config.clone(),
            storage,
            is_running: Arc::new(RwLock::new(false)),
            last_submitted_block: Arc::new(RwLock::new(0)),
        })
    }

    /// Start the batch submitter
    pub async fn start(&self) -> Result<()> {
        let mut running = self.is_running.write().await;
        if *running {
            return Ok(());
        }

        info!("Starting batch submitter");

        // Start the batch submission loop
        let self_clone = self.clone_for_task();
        tokio::spawn(async move {
            self_clone.batch_submission_loop().await;
        });

        *running = true;
        info!("Batch submitter started");

        Ok(())
    }

    /// Stop the batch submitter
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.is_running.write().await;
        if !*running {
            return Ok(());
        }

        info!("Stopping batch submitter");

        *running = false;
        info!("Batch submitter stopped");

        Ok(())
    }

    /// Main batch submission loop
    async fn batch_submission_loop(&self) {
        let mut interval = interval(Duration::from_secs(self.config.sequencer.batch_timeout));

        loop {
            interval.tick().await;

            // Check if we should stop
            if !*self.is_running.read().await {
                break;
            }

            // Try to submit a batch
            if let Err(e) = self.try_submit_batch().await {
                error!("Failed to submit batch: {}", e);
            }
        }
    }

    /// Try to submit a batch of transactions
    async fn try_submit_batch(&self) -> Result<()> {
        debug!("Checking for batch submission");

        // Get the latest block number
        let latest_block = self.storage.get_current_block_number().await?;
        let last_submitted = *self.last_submitted_block.read().await;

        // Check if we have enough blocks to submit
        let blocks_to_submit = latest_block.saturating_sub(last_submitted);

        if blocks_to_submit < self.config.sequencer.batch_size as u64 {
            debug!(
                "Not enough blocks for submission: {} < {}",
                blocks_to_submit, self.config.sequencer.batch_size
            );
            return Ok(());
        }

        // Collect blocks for the batch
        let start_block = last_submitted + 1;
        let end_block = start_block + self.config.sequencer.batch_size as u64 - 1;

        let blocks = self
            .collect_blocks_for_batch(start_block, end_block)
            .await?;

        if blocks.is_empty() {
            warn!("No blocks collected for batch");
            return Ok(());
        }

        // Create the batch
        let batch = self.create_batch(blocks).await?;

        // Submit to L1
        let l1_tx_hash = self.submit_batch_to_l1(&batch).await?;

        // Store batch information
        let mut batch_with_l1_hash = batch;
        batch_with_l1_hash.l1_tx_hash = Some(l1_tx_hash);

        self.storage.store_batch(&batch_with_l1_hash).await?;

        // Update last submitted block
        {
            let mut last_submitted = self.last_submitted_block.write().await;
            *last_submitted = end_block;
        }

        info!(
            "Batch submitted successfully: blocks {}-{}, L1 tx: {:?}",
            start_block, end_block, l1_tx_hash
        );

        Ok(())
    }

    /// Collect blocks for batch submission
    async fn collect_blocks_for_batch(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> Result<Vec<ArbitrumBlock>> {
        debug!("Collecting blocks {}-{} for batch", start_block, end_block);

        let mut blocks = Vec::new();

        for block_number in start_block..=end_block {
            if let Some(block) = self.storage.get_block_by_number(block_number).await? {
                blocks.push(block);
            } else {
                warn!("Block {} not found in storage", block_number);
                break;
            }
        }

        debug!("Collected {} blocks for batch", blocks.len());
        Ok(blocks)
    }

    /// Create a batch from the given blocks
    async fn create_batch(&self, blocks: Vec<ArbitrumBlock>) -> Result<ArbitrumBatch> {
        debug!("Creating batch from {} blocks", blocks.len());

        if blocks.is_empty() {
            return Err(eyre::eyre!("Cannot create batch from empty blocks"));
        }

        let start_block = blocks
            .first()
            .expect("Blocks vector should not be empty")
            .number;
        let end_block = blocks
            .last()
            .expect("Blocks vector should not be empty")
            .number;

        // Get the next batch number
        let batch_number = self.get_next_batch_number().await?;

        Ok(ArbitrumBatch {
            batch_number,
            block_range: (start_block, end_block),
            l1_block_number: 0, // Will be set when submitted to L1
            timestamp: chrono::Utc::now().timestamp() as u64,
            transactions: blocks.iter().flat_map(|b| b.transactions.clone()).collect(),
            l1_tx_hash: None, // Will be filled after L1 submission
        })
    }

    /// Calculate the batch root hash
    async fn calculate_batch_root(&self, blocks: &[ArbitrumBlock]) -> Result<B256> {
        // TODO: Implement proper Merkle root calculation
        // For now, use a simple hash of all block hashes

        use sha3::{Digest, Keccak256};
        let mut hasher = Keccak256::new();

        for block in blocks {
            hasher.update(block.hash.as_slice());
        }

        let result = hasher.finalize();
        Ok(B256::from_slice(&result))
    }

    /// Submit the batch to L1
    async fn submit_batch_to_l1(&self, batch: &ArbitrumBatch) -> Result<B256> {
        info!("Submitting batch {} to L1", batch.batch_number);

        // TODO: Implement actual L1 submission
        // This would involve:
        // 1. Encoding the batch data
        // 2. Creating an L1 transaction
        // 3. Signing and submitting the transaction
        // 4. Waiting for confirmation

        // For now, return a dummy transaction hash
        let dummy_hash = B256::from_slice(&[0u8; 32]);

        info!("Batch submitted to L1 with tx hash: {:?}", dummy_hash);
        Ok(dummy_hash)
    }

    /// Get the next batch number
    async fn get_next_batch_number(&self) -> Result<u64> {
        // TODO: Implement proper batch number tracking
        // This should query the last batch number from storage and increment
        Ok(1)
    }

    /// Get batch submission statistics
    pub async fn get_stats(&self) -> BatchSubmitterStats {
        let last_submitted = *self.last_submitted_block.read().await;
        let latest_block = self.storage.get_current_block_number().await.unwrap_or(0);

        BatchSubmitterStats {
            last_submitted_block: last_submitted,
            latest_block,
            pending_blocks: latest_block.saturating_sub(last_submitted),
            total_batches_submitted: 0, // TODO: Track this
        }
    }

    /// Check if the submitter should submit a batch based on time
    #[allow(dead_code)]
    async fn should_submit_by_time(&self) -> bool {
        // TODO: Implement time-based batch submission
        // This would check if enough time has passed since the last submission
        false
    }

    /// Check if the submitter should submit a batch based on size
    #[allow(dead_code)]
    async fn should_submit_by_size(&self) -> bool {
        let latest_block = self.storage.get_current_block_number().await.unwrap_or(0);
        let last_submitted = *self.last_submitted_block.read().await;
        let pending_blocks = latest_block.saturating_sub(last_submitted);

        pending_blocks >= self.config.sequencer.batch_size as u64
    }

    /// Force submit current pending blocks
    pub async fn force_submit(&self) -> Result<()> {
        info!("Force submitting current pending blocks");
        self.try_submit_batch().await
    }

    /// Helper method to clone for async tasks
    fn clone_for_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            storage: Arc::clone(&self.storage),
            is_running: Arc::clone(&self.is_running),
            last_submitted_block: Arc::clone(&self.last_submitted_block),
        }
    }
}

/// Batch submitter statistics
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BatchSubmitterStats {
    pub last_submitted_block: u64,
    pub latest_block: u64,
    pub pending_blocks: u64,
    pub total_batches_submitted: u64,
}

/// Batch submission result
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BatchSubmissionResult {
    pub batch_number: u64,
    pub l1_tx_hash: B256,
    pub start_block: u64,
    pub end_block: u64,
    pub gas_used: u64,
}
