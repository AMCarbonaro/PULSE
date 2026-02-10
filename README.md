# Pulse Network

**Proof-of-Life consensus for a human-backed global economy.**

> Your heartbeat is your mining rig.

## Overview

Pulse Network is a decentralized system where:
- **Heartbeats** replace hashing power for consensus
- **Tokens** are minted by being alive and active
- **Transactions** require proof of living human presence
- **Security** scales with global human participation

## Project Structure

```
Pulse/
├── docs/              # Whitepaper, architecture, specs
├── infrastructure/    # AWS IaC (Terraform)
├── prototype/         # Python proof-of-concept ✅
├── node/              # Rust production node ✅
├── device-sdk/        # Heartbeat capture SDK
│   └── ios/           # Swift SDK for iOS/watchOS ✅
├── app/               # Mobile/web frontend
└── scripts/           # Deployment, testing utilities
```

## Quick Start

### Run the Node

```bash
cd node
cargo build --release
./target/release/pulse-node --simulate
```

### API Endpoints

```bash
# Health check
curl http://localhost:8080/health

# Network stats
curl http://localhost:8080/stats

# Submit heartbeat (POST)
curl -X POST http://localhost:8080/pulse -H "Content-Type: application/json" -d '{...}'

# Get balance
curl http://localhost:8080/balance/{pubkey}
```

### iOS SDK

```swift
import PulseSDK

let client = try PulseClient(nodeURL: "http://your-node:8080")
try await client.requestAuthorization()
try await client.connect()
client.startPulsing()

print("Balance: \(client.balance) PULSE")
```

## Key Metrics (Projected)

| Scale | Active Users | TPS | Finality |
|-------|--------------|-----|----------|
| MVP | 1 | ~1 | Instant |
| Early | 1,000 | ~200 | ~10s |
| Growth | 100M | 1.8M | ~15s |
| Global | 1B | 18M | <10s |

## Status

- [x] Python prototype (consensus proof)
- [x] Rust node (production-ready)
- [x] iOS/watchOS SDK
- [ ] AWS deployment
- [ ] P2P multi-node sync
- [ ] Mobile app

---

*Life is proof. Pulse is currency.*
