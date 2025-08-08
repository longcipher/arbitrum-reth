use std::path::{Path, PathBuf};

use eyre::Result;
use serde::{Deserialize, Serialize};

/// Main configuration for Arbitrum-Reth node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrumRethConfig {
    pub node: NodeConfig,
    pub l1: L1Config,
    pub l2: L2Config,
    pub sequencer: SequencerConfig,
    pub validator: ValidatorConfig,
    pub network: NetworkConfig,
    pub metrics: MetricsConfig,
    pub logging: LoggingConfig,
    pub gas: GasConfig,
    pub rpc: RpcConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub chain: String,
    pub datadir: PathBuf,
    pub sequencer_mode: bool,
    pub validator_mode: bool,
    pub archive_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1Config {
    pub rpc_url: String,
    pub ws_url: Option<String>,
    pub chain_id: u64,
    pub confirmation_blocks: u64,
    pub poll_interval: u64,
    pub start_block: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L2Config {
    pub chain_id: u64,
    pub block_time: u64,
    pub max_tx_per_block: u64,
    pub gas_limit: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequencerConfig {
    pub enabled: bool,
    pub batch_size: usize,
    pub batch_timeout: u64,
    pub submit_interval: u64,
    pub max_batch_queue_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GasConfig {
    pub l1_base_fee: u64,
    pub l2_gas_price: u64,
    pub price_update_interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    pub port: u16,
    pub ws_port: u16,
    pub enable_ws: bool,
    pub cors_origins: Vec<String>,
    /// TTL for JSON-RPC log filters in milliseconds. 0 => use built-in default.
    #[serde(default)]
    pub filter_ttl_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorConfig {
    pub enable: bool,
    pub stake_amount: String,
    pub challenge_period: u64,
    pub max_challenge_depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub discovery_port: u16,
    pub listening_port: u16,
    pub max_peers: usize,
    pub bootnodes: Vec<String>,
    pub enable_mdns: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enable: bool,
    pub addr: String,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    pub file: Option<PathBuf>,
}

impl Default for ArbitrumRethConfig {
    fn default() -> Self {
        Self {
            node: NodeConfig {
                chain: "arbitrum-one".to_string(),
                datadir: PathBuf::from("./data"),
                sequencer_mode: false,
                validator_mode: false,
                archive_mode: false,
            },
            l1: L1Config {
                rpc_url: "https://ethereum.publicnode.com".to_string(),
                ws_url: None,
                chain_id: 1,
                confirmation_blocks: 6,
                poll_interval: 2000,
                start_block: 0,
            },
            l2: L2Config {
                chain_id: 42161,
                block_time: 250,
                max_tx_per_block: 1000,
                gas_limit: 32_000_000,
            },
            sequencer: SequencerConfig {
                enabled: false,
                batch_size: 100,
                batch_timeout: 10_000,
                submit_interval: 30_000,
                max_batch_queue_size: 1000,
            },
            validator: ValidatorConfig {
                enable: false,
                stake_amount: "1000000000000000000".to_string(), // 1 ETH
                challenge_period: 604_800,                       // 7 days
                max_challenge_depth: 32,
            },
            network: NetworkConfig {
                discovery_port: 30303,
                listening_port: 30304,
                max_peers: 50,
                bootnodes: vec![],
                enable_mdns: true,
            },
            metrics: MetricsConfig {
                enable: false,
                addr: "127.0.0.1:9090".to_string(),
                interval: 10,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "human".to_string(),
                file: None,
            },
            gas: GasConfig {
                l1_base_fee: 20_000_000_000, // 20 gwei
                l2_gas_price: 100_000_000,   // 0.1 gwei
                price_update_interval: 10,   // 10 seconds
            },
            rpc: RpcConfig {
                port: 8548,
                ws_port: 8549,
                enable_ws: true,
                cors_origins: vec!["*".to_string()],
                filter_ttl_ms: 0, // 0 => fallback to FiltersManager::DEFAULT_TTL_MILLIS
            },
        }
    }
}

impl ArbitrumRethConfig {
    /// Load configuration from a TOML file
    pub async fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await?;
        let config: ArbitrumRethConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to a TOML file
    #[allow(dead_code)]
    pub async fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        tokio::fs::write(path, content).await?;
        Ok(())
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Validate L1 configuration
        if self.l1.rpc_url.is_empty() {
            eyre::bail!("L1 RPC URL cannot be empty");
        }

        // Validate L2 configuration
        if self.l2.chain_id == 0 {
            eyre::bail!("L2 chain ID cannot be zero");
        }

        // Validate sequencer configuration
        if self.sequencer.enabled && self.sequencer.batch_size == 0 {
            eyre::bail!("Sequencer batch size cannot be zero");
        }

        // Validate validator configuration
        if self.validator.enable && self.validator.stake_amount.is_empty() {
            eyre::bail!("Validator stake amount cannot be empty");
        }

        // Validate network configuration
        if self.network.max_peers == 0 {
            eyre::bail!("Max peers cannot be zero");
        }

        Ok(())
    }

    /// Get the data directory for the specific chain
    pub fn chain_datadir(&self) -> PathBuf {
        self.node.datadir.join(&self.node.chain)
    }

    /// Get the database path
    pub fn db_path(&self) -> PathBuf {
        self.chain_datadir().join("db")
    }

    /// Get the static files path
    pub fn static_files_path(&self) -> PathBuf {
        self.chain_datadir().join("static_files")
    }
}
