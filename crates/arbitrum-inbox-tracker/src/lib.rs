#![allow(dead_code)]

use std::{collections::VecDeque, sync::Arc, time::Duration};

use alloy_primitives::{Address, B256};
use arbitrum_config::ArbitrumRethConfig;
use arbitrum_storage::{ArbitrumStorage, L1Message};
use eyre::Result;
use tokio::{sync::RwLock, time::interval};
use tracing::{debug, error, info};

/// Inbox tracker responsible for monitoring L1 for new messages and batches
pub struct InboxTracker {
    config: ArbitrumRethConfig,
    storage: Arc<ArbitrumStorage>,
    is_running: Arc<RwLock<bool>>,
    last_processed_l1_block: Arc<RwLock<u64>>,
    pending_messages: Arc<RwLock<VecDeque<L1Message>>>,
    // TODO: Add L1 client for monitoring
    // l1_client: Arc<dyn L1Client>,
}

impl InboxTracker {
    /// Create a new inbox tracker
    pub async fn new(config: &ArbitrumRethConfig, storage: Arc<ArbitrumStorage>) -> Result<Self> {
        info!("Initializing inbox tracker");

        Ok(Self {
            config: config.clone(),
            storage,
            is_running: Arc::new(RwLock::new(false)),
            last_processed_l1_block: Arc::new(RwLock::new(0)),
            pending_messages: Arc::new(RwLock::new(VecDeque::new())),
        })
    }

    /// Start the inbox tracker
    pub async fn start(&self) -> Result<()> {
        let mut running = self.is_running.write().await;
        if *running {
            return Ok(());
        }

        info!("Starting inbox tracker");

        // Initialize the last processed block from storage or config
        self.initialize_last_processed_block().await?;

        // Start the L1 monitoring loop
        let self_clone = self.clone_for_task();
        tokio::spawn(async move {
            self_clone.l1_monitoring_loop().await;
        });

        // Start the message processing loop
        let self_clone = self.clone_for_task();
        tokio::spawn(async move {
            self_clone.message_processing_loop().await;
        });

        *running = true;
        info!("Inbox tracker started");

        Ok(())
    }

    /// Stop the inbox tracker
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.is_running.write().await;
        if !*running {
            return Ok(());
        }

        info!("Stopping inbox tracker");

        *running = false;
        info!("Inbox tracker stopped");

        Ok(())
    }

    /// Initialize the last processed L1 block number
    async fn initialize_last_processed_block(&self) -> Result<()> {
        // TODO: Load from storage or use config default
        let start_block = self.config.l1.start_block;

        let mut last_processed = self.last_processed_l1_block.write().await;
        *last_processed = start_block;

        info!("Initialized last processed L1 block: {}", start_block);
        Ok(())
    }

    /// Main L1 monitoring loop
    async fn l1_monitoring_loop(&self) {
        let mut interval = interval(Duration::from_secs(5)); // Check L1 every 5 seconds

        loop {
            interval.tick().await;

            // Check if we should stop
            if !*self.is_running.read().await {
                break;
            }

            // Process new L1 blocks
            if let Err(e) = self.process_new_l1_blocks().await {
                error!("Failed to process new L1 blocks: {}", e);
            }
        }
    }

    /// Message processing loop
    async fn message_processing_loop(&self) {
        let mut interval = interval(Duration::from_millis(100)); // Process messages every 100ms

        loop {
            interval.tick().await;

            // Check if we should stop
            if !*self.is_running.read().await {
                break;
            }

            // Process pending messages
            if let Err(e) = self.process_pending_messages().await {
                error!("Failed to process pending messages: {}", e);
            }
        }
    }

    /// Process new L1 blocks for inbox events
    async fn process_new_l1_blocks(&self) -> Result<()> {
        // TODO: Get latest L1 block number from L1 client
        let latest_l1_block = self.get_latest_l1_block().await?;
        let last_processed = *self.last_processed_l1_block.read().await;

        if latest_l1_block <= last_processed {
            return Ok(());
        }

        debug!(
            "Processing L1 blocks {}-{}",
            last_processed + 1,
            latest_l1_block
        );

        // Process each new block
        for block_number in (last_processed + 1)..=latest_l1_block {
            self.process_l1_block(block_number).await?;
        }

        // Update last processed block
        {
            let mut last_processed = self.last_processed_l1_block.write().await;
            *last_processed = latest_l1_block;
        }

        Ok(())
    }

    /// Process a single L1 block for inbox events
    async fn process_l1_block(&self, block_number: u64) -> Result<()> {
        debug!("Processing L1 block: {}", block_number);

        // TODO: Get block data from L1 client
        let block = self.get_l1_block(block_number).await?;

        // Look for inbox-related events
        for event in block.events {
            match event.event_type {
                L1EventType::MessageSent => {
                    let message = self.parse_message_sent_event(&event).await?;
                    self.add_pending_message(message).await?;
                }
                L1EventType::BatchSubmitted => {
                    self.handle_batch_submitted_event(&event).await?;
                }
                L1EventType::StateUpdated => {
                    self.handle_state_updated_event(&event).await?;
                }
                _ => {
                    // Ignore other events
                }
            }
        }

        Ok(())
    }

    /// Parse a MessageSent event into an L1Message
    async fn parse_message_sent_event(&self, event: &L1Event) -> Result<L1Message> {
        // TODO: Parse the actual event data
        // This is a simplified implementation

        Ok(L1Message {
            message_number: event.message_number,
            sender: event.sender,
            data: event.data.clone(),
            timestamp: event.timestamp,
            block_number: event.block_number,
        })
    }

    /// Handle a BatchSubmitted event
    async fn handle_batch_submitted_event(&self, event: &L1Event) -> Result<()> {
        debug!("Handling BatchSubmitted event: {:?}", event);

        // TODO: Process batch submission
        // This would involve:
        // 1. Validating the batch
        // 2. Updating local state
        // 3. Triggering any necessary actions

        Ok(())
    }

    /// Handle a StateUpdated event
    async fn handle_state_updated_event(&self, event: &L1Event) -> Result<()> {
        debug!("Handling StateUpdated event: {:?}", event);

        // TODO: Process state update
        // This would involve:
        // 1. Validating the state update
        // 2. Updating local state
        // 3. Checking for conflicts

        Ok(())
    }

    /// Add a message to the pending queue
    async fn add_pending_message(&self, message: L1Message) -> Result<()> {
        debug!("Adding pending message: {}", message.message_number);

        {
            let mut pending = self.pending_messages.write().await;
            pending.push_back(message.clone());
        }

        // Store the message in persistent storage
        self.storage.store_l1_message(&message).await?;

        Ok(())
    }

    /// Process pending messages
    async fn process_pending_messages(&self) -> Result<()> {
        let message = {
            let mut pending = self.pending_messages.write().await;
            pending.pop_front()
        };

        if let Some(message) = message {
            self.process_l1_message(message).await?;
        }

        Ok(())
    }

    /// Process a single L1 message
    async fn process_l1_message(&self, message: L1Message) -> Result<()> {
        debug!("Processing L1 message: {}", message.message_number);

        // TODO: Process the message based on its type
        // This could involve:
        // 1. Creating L2 transactions
        // 2. Updating contract state
        // 3. Handling deposits/withdrawals

        info!("Processed L1 message: {}", message.message_number);
        Ok(())
    }

    /// Get the latest L1 block number
    async fn get_latest_l1_block(&self) -> Result<u64> {
        // TODO: Get from actual L1 client
        Ok(1000) // Dummy value
    }

    /// Get L1 block data
    async fn get_l1_block(&self, block_number: u64) -> Result<L1Block> {
        // TODO: Get from actual L1 client
        Ok(L1Block {
            number: block_number,
            hash: B256::ZERO,
            timestamp: chrono::Utc::now().timestamp() as u64,
            events: vec![], // Dummy empty events
        })
    }

    /// Get inbox tracker statistics
    pub async fn get_stats(&self) -> InboxTrackerStats {
        let last_processed = *self.last_processed_l1_block.read().await;
        let pending_count = self.pending_messages.read().await.len();
        let latest_l1_block = self.get_latest_l1_block().await.unwrap_or(0);

        InboxTrackerStats {
            last_processed_l1_block: last_processed,
            latest_l1_block,
            blocks_behind: latest_l1_block.saturating_sub(last_processed),
            pending_messages: pending_count,
            total_messages_processed: 0, // TODO: Track this
        }
    }

    /// Get the next message number to be processed
    pub async fn get_next_message_number(&self) -> u64 {
        // TODO: Track message numbers properly
        0
    }

    /// Force process all pending messages
    pub async fn force_process_pending(&self) -> Result<usize> {
        let mut processed = 0;

        while !self.pending_messages.read().await.is_empty() {
            if let Err(e) = self.process_pending_messages().await {
                error!("Failed to process pending message: {}", e);
                break;
            }
            processed += 1;
        }

        info!("Force processed {} pending messages", processed);
        Ok(processed)
    }

    /// Helper method to clone for async tasks
    fn clone_for_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            storage: Arc::clone(&self.storage),
            is_running: Arc::clone(&self.is_running),
            last_processed_l1_block: Arc::clone(&self.last_processed_l1_block),
            pending_messages: Arc::clone(&self.pending_messages),
        }
    }
}

/// Represents an L1 block with relevant events
#[derive(Debug, Clone)]
pub struct L1Block {
    pub number: u64,
    pub hash: B256,
    pub timestamp: u64,
    pub events: Vec<L1Event>,
}

/// Represents an L1 event relevant to the inbox
#[derive(Debug, Clone)]
pub struct L1Event {
    pub event_type: L1EventType,
    pub message_number: u64,
    pub sender: Address,
    pub data: Vec<u8>,
    pub timestamp: u64,
    pub block_number: u64,
    pub transaction_hash: B256,
}

/// Types of L1 events we track
#[derive(Debug, Clone, PartialEq)]
pub enum L1EventType {
    MessageSent,
    BatchSubmitted,
    StateUpdated,
    ChallengeCreated,
    ChallengeResolved,
    Other,
}

/// Inbox tracker statistics
#[derive(Debug, Clone)]
pub struct InboxTrackerStats {
    pub last_processed_l1_block: u64,
    pub latest_l1_block: u64,
    pub blocks_behind: u64,
    pub pending_messages: usize,
    pub total_messages_processed: u64,
}
