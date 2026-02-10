# Pulse Network Architecture

## System Layers

```
[1] Heartbeat Capture (Device Layer)
         │
         ▼
[2] Broadcast & Node Reception (Node Layer)
         │
         ▼
[3] Transaction Collection (Application Layer)
         │
         ▼
[4] Pulse Block Assembly (Pulse Block Layer)
         │
         ▼
[5] Consensus Verification (Proof-of-Life Layer)
         │
         ▼
[6] Commit Pulse Block (Chain Layer)
         │
         ▼
[7] Broadcast Pulse Block (Network Layer)
         │
         ▼
[8] Application / Rewards Layer
         │
         ▼
[9] Persistent Web4 Ledger
```

---

## Layer Details

### [1] Device Layer
- Sensors capture: heartbeat (primary), motion, temperature
- Device signs heartbeat data with private key
- Output: `{timestamp, heart_rate, motion, device_pubkey, signature}`

### [2] Node Layer
- Verify signature, timestamp, continuity
- Reject replayed or invalid heartbeats
- Maintain mempool of active pulses

### [3] Application Layer
- Users perform actions (send tokens, play games, social)
- Each action includes pulse signature
- Only life-verified transactions proceed

### [4] Pulse Block Layer
- Aggregate verified heartbeats + transactions
- Block header: prev_hash, timestamp, activity metrics

### [5] Proof-of-Life Layer
- Check minimum active heartbeats (N_threshold)
- Verify all signatures, no duplicates
- Block approved only if life threshold met

### [6] Chain Layer
- Append block to chain
- Update ledger state (token balances, activity)
- Chain reorg: select block with highest cumulative life

### [7] Network Layer
- Gossip protocol propagation
- Global sync across nodes
- P2P communication (libp2p/gRPC)

### [8] Rewards Layer
- Distribute Pulse tokens based on W_i
- Game points, reputation scores
- Verified identity/presence

### [9] Ledger
- Immutable history of human presence
- Transactions and activity logs
- Publicly verifiable

---

## Node Types

| Type | Role |
|------|------|
| Full Node | Complete chain, validates all |
| Light Node | Recent blocks, Merkle proofs |
| Validator Node | Focuses on PoL verification |
| Relay Node | Propagates heartbeats/blocks |

---

## Data Structures

### Heartbeat Packet
```python
heartbeat = {
    "timestamp": 1675987654,
    "heart_rate": 72,
    "motion": {"x": 0.2, "y": -0.1, "z": 0.05},
    "temperature": 36.7,
    "device_pubkey": "ABC123",
    "signature": "XYZ789"
}
```

### Transaction
```python
transaction = {
    "sender_pubkey": "ABC123",
    "recipient_pubkey": "DEF456",
    "amount": 10,
    "heartbeat_signature": "XYZ789",
    "timestamp": 1675987654,
    "transaction_id": "tx_0001"
}
```

### Pulse Block
```python
block = {
    "previous_block_hash": "...",
    "timestamp": 1675987654,
    "heartbeats": [...],
    "transactions": [...],
    "block_signature": "..."
}
```
