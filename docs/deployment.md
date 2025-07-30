# Arbitrum-Reth Deployment Guide

This guide provides comprehensive instructions for deploying and operating Arbitrum-Reth nodes in various environments.

## Table of Contents

1. [System Requirements](#system-requirements)
2. [Pre-deployment Checklist](#pre-deployment-checklist)
3. [Installation Methods](#installation-methods)
4. [Configuration](#configuration)
5. [Deployment Scenarios](#deployment-scenarios)
6. [Monitoring and Observability](#monitoring-and-observability)
7. [Maintenance and Upgrades](#maintenance-and-upgrades)
8. [Troubleshooting](#troubleshooting)
9. [Security Considerations](#security-considerations)

## System Requirements

### Minimum Requirements

| Component | Specification |
|-----------|---------------|
| CPU | 4 cores, 2.4 GHz |
| RAM | 16 GB |
| Storage | 500 GB SSD |
| Network | 25 Mbps bandwidth |
| OS | Ubuntu 20.04+, macOS 12+, Windows 11 |

### Recommended Requirements

| Component | Specification |
|-----------|---------------|
| CPU | 8 cores, 3.0 GHz |
| RAM | 32 GB |
| Storage | 1 TB NVMe SSD |
| Network | 100 Mbps bandwidth |
| OS | Ubuntu 22.04 LTS |

### High-Performance Requirements

| Component | Specification |
|-----------|---------------|
| CPU | 16 cores, 3.5 GHz |
| RAM | 64 GB |
| Storage | 2 TB NVMe SSD (RAID 0) |
| Network | 1 Gbps bandwidth |
| OS | Ubuntu 22.04 LTS |

## Pre-deployment Checklist

### Environment Preparation

- [ ] System meets minimum requirements
- [ ] Operating system is up to date
- [ ] Rust toolchain installed (1.88.0+)
- [ ] Git installed and configured
- [ ] Network connectivity verified
- [ ] Firewall configured appropriately
- [ ] Sufficient disk space available
- [ ] Backup strategy planned

### Dependencies

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev git curl

# macOS (with Homebrew)
brew install openssl pkg-config git

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### Network Configuration

Ensure the following ports are accessible:

| Port | Protocol | Purpose | Required |
|------|----------|---------|----------|
| 30303 | TCP/UDP | P2P Discovery | Yes |
| 30304 | TCP | P2P Listening | Yes |
| 8545 | TCP | RPC HTTP | Optional |
| 8546 | TCP | RPC WebSocket | Optional |
| 8551 | TCP | Engine API | Yes (Sequencer) |
| 9090 | TCP | Metrics | Optional |

## Installation Methods

### Method 1: Build from Source

```bash
# Clone the repository
git clone https://github.com/longcipher/arbitrum-reth.git
cd arbitrum-reth

# Build release binary
cargo build --release --features "jemalloc asm-keccak"

# Install to system path (optional)
sudo cp target/release/arbitrum-reth /usr/local/bin/
```

### Method 2: Docker Deployment

```dockerfile
# Dockerfile
FROM rust:1.88 as builder

WORKDIR /app
COPY . .
RUN cargo build --release --features "jemalloc asm-keccak"

FROM ubuntu:22.04
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/arbitrum-reth /usr/local/bin/
EXPOSE 30303 30304 8545 8546 8551 9090
ENTRYPOINT ["arbitrum-reth"]
```

```bash
# Build Docker image
docker build -t arbitrum-reth:latest .

# Run container
docker run -d \
  --name arbitrum-reth \
  -p 30303:30303/udp \
  -p 30304:30304 \
  -p 8545:8545 \
  -p 9090:9090 \
  -v /path/to/data:/data \
  -v /path/to/config:/config \
  arbitrum-reth:latest \
  --config /config/arbitrum-reth.toml \
  --datadir /data
```

### Method 3: Docker Compose

```yaml
# docker-compose.yml
version: '3.8'

services:
  arbitrum-reth:
    build: .
    container_name: arbitrum-reth
    restart: unless-stopped
    ports:
      - "30303:30303/udp"
      - "30304:30304"
      - "8545:8545"
      - "8546:8546"
      - "9090:9090"
    volumes:
      - ./data:/data
      - ./config:/config
      - ./logs:/logs
    command: >
      --config /config/arbitrum-reth.toml
      --datadir /data
      --log-level info
      --metrics
      --metrics-addr 0.0.0.0:9090
    environment:
      - RUST_LOG=info
      - RUST_BACKTRACE=1
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:9090/metrics"]
      interval: 30s
      timeout: 10s
      retries: 3

  prometheus:
    image: prom/prometheus:latest
    container_name: prometheus
    ports:
      - "9091:9090"
    volumes:
      - ./monitoring/prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.console.libraries=/etc/prometheus/console_libraries'
      - '--web.console.templates=/etc/prometheus/consoles'

  grafana:
    image: grafana/grafana:latest
    container_name: grafana
    ports:
      - "3000:3000"
    volumes:
      - grafana_data:/var/lib/grafana
      - ./monitoring/grafana:/etc/grafana/provisioning
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin

volumes:
  prometheus_data:
  grafana_data:
```

## Configuration

### Basic Configuration

Create `config.toml`:

```toml
[node]
chain = "arbitrum-one"
datadir = "/data/arbitrum-one"
sequencer_mode = false
validator_mode = false
archive_mode = false

[l1]
rpc_url = "https://ethereum.publicnode.com"
ws_url = "wss://ethereum.publicnode.com"
chain_id = 1
confirmation_blocks = 6
poll_interval = 2000
start_block = 0

[l2]
chain_id = 42161
block_time = 250
max_tx_per_block = 1000
gas_limit = 32000000

[sequencer]
enable = false
batch_size = 100
batch_timeout = 10000
submit_interval = 30000
max_batch_queue_size = 1000

[validator]
enable = false
stake_amount = "1000000000000000000"
challenge_period = 604800
max_challenge_depth = 32

[network]
discovery_port = 30303
listening_port = 30304
max_peers = 50
bootnodes = []
enable_mdns = true

[rpc]
http_enabled = true
http_addr = "0.0.0.0"
http_port = 8545
http_api = ["eth", "net", "web3", "arb"]
ws_enabled = true
ws_addr = "0.0.0.0"
ws_port = 8546

[metrics]
enable = true
addr = "0.0.0.0:9090"
interval = 10

[logging]
level = "info"
format = "json"
file = "/logs/arbitrum-reth.log"
```

### Environment-Specific Configurations

#### Development Environment

```toml
[node]
datadir = "./dev-data"

[l1]
rpc_url = "http://localhost:8545"  # Local Ethereum node

[logging]
level = "debug"
format = "human"

[metrics]
addr = "127.0.0.1:9090"
```

#### Testnet Environment

```toml
[node]
chain = "arbitrum-sepolia"

[l1]
rpc_url = "https://sepolia.infura.io/v3/YOUR_PROJECT_ID"
chain_id = 11155111

[l2]
chain_id = 421614
```

#### Production Environment

```toml
[node]
archive_mode = true

[l1]
rpc_url = "https://mainnet.infura.io/v3/YOUR_PROJECT_ID"
ws_url = "wss://mainnet.infura.io/ws/v3/YOUR_PROJECT_ID"

[logging]
level = "warn"
format = "json"
file = "/var/log/arbitrum-reth/arbitrum-reth.log"

[metrics]
enable = true
addr = "127.0.0.1:9090"
```

## Deployment Scenarios

### Scenario 1: Read-Only Node

For applications that only need to read blockchain data.

```bash
arbitrum-reth \
  --config /etc/arbitrum-reth/config.toml \
  --datadir /var/lib/arbitrum-reth \
  --log-level info
```

### Scenario 2: Sequencer Node

For operators running a sequencer.

```toml
[sequencer]
enable = true
batch_size = 100
batch_timeout = 10000
submit_interval = 30000

[l1]
# Requires a reliable L1 connection for batch submission
rpc_url = "https://mainnet.infura.io/v3/YOUR_PROJECT_ID"
```

```bash
arbitrum-reth \
  --config /etc/arbitrum-reth/sequencer-config.toml \
  --sequencer \
  --datadir /var/lib/arbitrum-reth \
  --metrics \
  --metrics-addr 0.0.0.0:9090
```

### Scenario 3: Validator Node

For operators participating in validation.

```toml
[validator]
enable = true
stake_amount = "10000000000000000000"  # 10 ETH
challenge_period = 604800
```

```bash
arbitrum-reth \
  --config /etc/arbitrum-reth/validator-config.toml \
  --validator \
  --datadir /var/lib/arbitrum-reth
```

### Scenario 4: High Availability Setup

For production deployments requiring high availability.

```yaml
# docker-compose.ha.yml
version: '3.8'

services:
  arbitrum-reth-primary:
    build: .
    container_name: arbitrum-reth-primary
    volumes:
      - ./data-primary:/data
    ports:
      - "8545:8545"
      - "30303:30303/udp"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8545"]
      interval: 30s
      timeout: 10s
      retries: 3

  arbitrum-reth-replica:
    build: .
    container_name: arbitrum-reth-replica
    volumes:
      - ./data-replica:/data
    ports:
      - "8546:8545"
      - "30304:30303/udp"
    profiles: ["replica"]

  haproxy:
    image: haproxy:latest
    container_name: haproxy
    ports:
      - "80:80"
      - "8080:8080"
    volumes:
      - ./haproxy.cfg:/usr/local/etc/haproxy/haproxy.cfg
    depends_on:
      - arbitrum-reth-primary
```

## Monitoring and Observability

### Metrics Collection

Arbitrum-Reth exposes Prometheus metrics on the configured metrics endpoint.

```bash
# Check metrics endpoint
curl http://localhost:9090/metrics
```

### Key Metrics to Monitor

| Metric | Description | Alert Threshold |
|--------|-------------|-----------------|
| `node_sync_status` | Sync status (0=syncing, 1=synced) | < 1 |
| `node_block_height` | Current block height | Lagging behind network |
| `tx_pool_size` | Transaction pool size | > 10000 |
| `memory_usage_bytes` | Memory usage | > 80% of available |
| `disk_usage_percent` | Disk usage percentage | > 85% |
| `peer_count` | Number of connected peers | < 5 |
| `rpc_requests_total` | Total RPC requests | Rate monitoring |
| `batch_submission_success` | Successful batch submissions | < 95% |

### Prometheus Configuration

```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'arbitrum-reth'
    static_configs:
      - targets: ['localhost:9090']
    scrape_interval: 5s
    metrics_path: /metrics

rule_files:
  - "arbitrum-reth.rules.yml"

alerting:
  alertmanagers:
    - static_configs:
        - targets:
          - alertmanager:9093
```

### Grafana Dashboard

Sample dashboard configuration for monitoring Arbitrum-Reth nodes.

```json
{
  "dashboard": {
    "title": "Arbitrum-Reth Node Monitoring",
    "panels": [
      {
        "title": "Sync Status",
        "type": "stat",
        "targets": [
          {
            "expr": "node_sync_status",
            "legendFormat": "Sync Status"
          }
        ]
      },
      {
        "title": "Block Height",
        "type": "graph",
        "targets": [
          {
            "expr": "node_block_height",
            "legendFormat": "Current Height"
          }
        ]
      },
      {
        "title": "Memory Usage",
        "type": "graph",
        "targets": [
          {
            "expr": "memory_usage_bytes",
            "legendFormat": "Memory Usage"
          }
        ]
      }
    ]
  }
}
```

### Log Management

Configure structured logging for better observability:

```toml
[logging]
level = "info"
format = "json"
file = "/var/log/arbitrum-reth/arbitrum-reth.log"
```

Use log aggregation tools like ELK stack or Loki:

```yaml
# docker-compose.logging.yml
version: '3.8'

services:
  loki:
    image: grafana/loki:latest
    ports:
      - "3100:3100"
    volumes:
      - ./loki-config.yaml:/etc/loki/local-config.yaml

  promtail:
    image: grafana/promtail:latest
    volumes:
      - ./logs:/var/log
      - ./promtail-config.yaml:/etc/promtail/config.yml
```

## Maintenance and Upgrades

### Regular Maintenance Tasks

#### Daily Tasks

- Monitor system health and metrics
- Check log files for errors
- Verify synchronization status
- Monitor disk usage

#### Weekly Tasks

- Review performance metrics
- Check for software updates
- Backup configuration files
- Clean up old log files

#### Monthly Tasks

- Update system packages
- Review security settings
- Performance tuning
- Capacity planning

### Upgrade Procedures

#### Planning Phase

1. Review release notes
2. Test upgrade in staging environment
3. Plan maintenance window
4. Notify users of downtime
5. Prepare rollback plan

#### Upgrade Process

```bash
# 1. Stop the node gracefully
sudo systemctl stop arbitrum-reth

# 2. Backup current installation
sudo cp /usr/local/bin/arbitrum-reth /usr/local/bin/arbitrum-reth.backup
sudo cp -r /var/lib/arbitrum-reth /var/lib/arbitrum-reth.backup

# 3. Download and install new version
wget https://github.com/longcipher/arbitrum-reth/releases/download/v1.x.x/arbitrum-reth
sudo chmod +x arbitrum-reth
sudo mv arbitrum-reth /usr/local/bin/

# 4. Update configuration if needed
sudo cp config.toml.new /etc/arbitrum-reth/config.toml

# 5. Start the node
sudo systemctl start arbitrum-reth

# 6. Verify operation
sudo systemctl status arbitrum-reth
arbitrum-reth --version
```

#### Rollback Process

```bash
# If upgrade fails, rollback to previous version
sudo systemctl stop arbitrum-reth
sudo cp /usr/local/bin/arbitrum-reth.backup /usr/local/bin/arbitrum-reth
sudo rm -rf /var/lib/arbitrum-reth
sudo mv /var/lib/arbitrum-reth.backup /var/lib/arbitrum-reth
sudo systemctl start arbitrum-reth
```

### Database Maintenance

#### Pruning Old Data

```bash
# Prune old blocks (if not running in archive mode)
arbitrum-reth db prune --datadir /var/lib/arbitrum-reth --blocks-before 1000000

# Compact database
arbitrum-reth db compact --datadir /var/lib/arbitrum-reth
```

#### Backup Procedures

```bash
# Create snapshot backup
sudo tar -czf arbitrum-reth-backup-$(date +%Y%m%d).tar.gz -C /var/lib arbitrum-reth

# Incremental backup (using rsync)
rsync -av --delete /var/lib/arbitrum-reth/ /backup/arbitrum-reth/
```

## Troubleshooting

### Common Issues

#### Node Won't Start

**Symptoms**: Process exits immediately
**Causes**: Configuration errors, port conflicts, permission issues
**Solutions**:

```bash
# Check configuration
arbitrum-reth --config config.toml check

# Check port availability
netstat -tulpn | grep :30303

# Check permissions
ls -la /var/lib/arbitrum-reth
sudo chown -R arbitrum:arbitrum /var/lib/arbitrum-reth
```

#### Sync Issues

**Symptoms**: Node falls behind or stops syncing
**Causes**: Network issues, peer problems, disk space
**Solutions**:

```bash
# Check sync status
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_syncing","params":[],"id":1}' \
  http://localhost:8545

# Check peer count
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"net_peerCount","params":[],"id":1}' \
  http://localhost:8545

# Restart sync from checkpoint
arbitrum-reth --datadir /var/lib/arbitrum-reth resync --from-block 1000000
```

#### Performance Issues

**Symptoms**: Slow response times, high CPU/memory usage
**Solutions**:

```bash
# Check system resources
htop
iostat -x 1

# Optimize configuration
[node]
cache_size = 2048  # Increase cache
max_connections = 100  # Limit connections

# Enable performance features
cargo build --release --features "jemalloc asm-keccak"
```

### Log Analysis

#### Important Log Patterns

```bash
# Check for errors
grep "ERROR" /var/log/arbitrum-reth/arbitrum-reth.log

# Monitor sync progress
grep "sync_progress" /var/log/arbitrum-reth/arbitrum-reth.log | tail -10

# Check for network issues
grep "peer" /var/log/arbitrum-reth/arbitrum-reth.log | grep -E "(disconnect|error)"
```

#### Debugging Commands

```bash
# Enable debug logging
arbitrum-reth --log-level debug

# Trace specific components
RUST_LOG=arbitrum_consensus=trace,arbitrum_pool=debug arbitrum-reth

# Generate debug dump
arbitrum-reth debug --datadir /var/lib/arbitrum-reth --output debug-dump.json
```

## Security Considerations

### Network Security

#### Firewall Configuration

```bash
# Ubuntu/Debian (ufw)
sudo ufw allow 30303/udp comment "Arbitrum-Reth P2P Discovery"
sudo ufw allow 30304/tcp comment "Arbitrum-Reth P2P Listening"
sudo ufw allow from 10.0.0.0/8 to any port 8545 comment "RPC (internal only)"
sudo ufw allow from 10.0.0.0/8 to any port 9090 comment "Metrics (internal only)"

# CentOS/RHEL (firewalld)
sudo firewall-cmd --add-port=30303/udp --permanent
sudo firewall-cmd --add-port=30304/tcp --permanent
sudo firewall-cmd --add-rich-rule='rule family="ipv4" source address="10.0.0.0/8" port protocol="tcp" port="8545" accept' --permanent
sudo firewall-cmd --reload
```

#### TLS Configuration

```toml
[rpc]
tls_enabled = true
tls_cert_file = "/etc/ssl/certs/arbitrum-reth.crt"
tls_key_file = "/etc/ssl/private/arbitrum-reth.key"
```

### Access Control

#### JWT Authentication

```toml
[rpc]
jwt_enabled = true
jwt_secret_file = "/etc/arbitrum-reth/jwt.secret"
```

#### Rate Limiting

```toml
[rpc]
rate_limit_enabled = true
rate_limit_requests_per_minute = 1000
rate_limit_burst_size = 100
```

### Data Protection

#### Encryption at Rest

```bash
# Enable disk encryption
sudo cryptsetup luksFormat /dev/sdb
sudo cryptsetup luksOpen /dev/sdb arbitrum-data
sudo mkfs.ext4 /dev/mapper/arbitrum-data
```

#### Backup Security

```bash
# Encrypt backups
gpg --symmetric --cipher-algo AES256 arbitrum-reth-backup.tar.gz

# Secure backup storage
aws s3 cp arbitrum-reth-backup.tar.gz.gpg s3://secure-backup-bucket/ \
  --server-side-encryption AES256
```

### Monitoring Security

#### Security Metrics

```yaml
# security-rules.yml
groups:
  - name: arbitrum-reth-security
    rules:
      - alert: UnauthorizedAccess
        expr: rate(rpc_requests_total{status_code!~"2.."}[5m]) > 10
        for: 1m
        labels:
          severity: warning
        annotations:
          summary: "High rate of failed RPC requests"

      - alert: SuspiciousActivity
        expr: increase(peer_disconnections_total[1h]) > 100
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Unusual peer disconnection rate"
```

#### Intrusion Detection

```bash
# Install and configure fail2ban
sudo apt install fail2ban

# Configure jail for arbitrum-reth
cat > /etc/fail2ban/jail.local << EOF
[arbitrum-reth]
enabled = true
port = 8545
filter = arbitrum-reth
logpath = /var/log/arbitrum-reth/arbitrum-reth.log
maxretry = 5
bantime = 3600
EOF
```

---

This deployment guide provides comprehensive instructions for setting up and maintaining Arbitrum-Reth nodes in production environments. Regular updates and monitoring are essential for optimal performance and security.
