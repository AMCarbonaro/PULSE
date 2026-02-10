#!/usr/bin/env python3
"""
Pulse Network - Proof of Concept
A working prototype of Proof-of-Life consensus.

Run: python pulse_network.py
"""

import hashlib
import json
import time
import random
from dataclasses import dataclass, field, asdict
from typing import List, Dict, Optional
from ecdsa import SigningKey, VerifyingKey, SECP256k1, BadSignatureError
import math


# =============================================================================
# CRYPTO LAYER
# =============================================================================

def generate_keypair():
    """Generate ECDSA keypair for device/user identity."""
    sk = SigningKey.generate(curve=SECP256k1)
    vk = sk.get_verifying_key()
    return sk.to_string().hex(), vk.to_string().hex()


def sign_data(private_key_hex: str, data: dict) -> str:
    """Sign data with private key."""
    sk = SigningKey.from_string(bytes.fromhex(private_key_hex), curve=SECP256k1)
    message = json.dumps(data, sort_keys=True).encode()
    signature = sk.sign(message)
    return signature.hex()


def verify_signature(public_key_hex: str, data: dict, signature_hex: str) -> bool:
    """Verify signature against public key."""
    try:
        vk = VerifyingKey.from_string(bytes.fromhex(public_key_hex), curve=SECP256k1)
        message = json.dumps(data, sort_keys=True).encode()
        return vk.verify(bytes.fromhex(signature_hex), message)
    except BadSignatureError:
        return False


def hash_block(block: dict) -> str:
    """SHA-256 hash of block contents."""
    return hashlib.sha256(json.dumps(block, sort_keys=True).encode()).hexdigest()


# =============================================================================
# DATA STRUCTURES
# =============================================================================

@dataclass
class Heartbeat:
    """A single heartbeat packet from a device."""
    timestamp: float
    heart_rate: int          # BPM
    motion: Dict[str, float] # {x, y, z}
    temperature: float
    device_pubkey: str
    signature: str = ""
    
    def to_signable(self) -> dict:
        """Data that gets signed (excludes signature itself)."""
        return {
            "timestamp": self.timestamp,
            "heart_rate": self.heart_rate,
            "motion": self.motion,
            "temperature": self.temperature,
            "device_pubkey": self.device_pubkey
        }
    
    def weight(self, alpha=0.4, beta=0.4, gamma=0.2) -> float:
        """
        Calculate weighted contribution W_i = α·HR + β·||M|| + γ·continuity
        Normalized to ~1.0 for average human.
        """
        hr_norm = self.heart_rate / 70.0  # Normalize around resting HR
        motion_mag = math.sqrt(
            self.motion["x"]**2 + 
            self.motion["y"]**2 + 
            self.motion["z"]**2
        )
        motion_norm = min(motion_mag / 0.5, 2.0)  # Cap at 2x for high activity
        continuity = 1.0  # Placeholder - would track gaps in real system
        
        return alpha * hr_norm + beta * motion_norm + gamma * continuity


@dataclass
class Transaction:
    """A pulse-backed transaction."""
    tx_id: str
    sender_pubkey: str
    recipient_pubkey: str
    amount: float
    timestamp: float
    heartbeat_signature: str  # Must reference a valid, recent heartbeat
    signature: str = ""
    
    def to_signable(self) -> dict:
        return {
            "tx_id": self.tx_id,
            "sender_pubkey": self.sender_pubkey,
            "recipient_pubkey": self.recipient_pubkey,
            "amount": self.amount,
            "timestamp": self.timestamp,
            "heartbeat_signature": self.heartbeat_signature
        }


@dataclass
class PulseBlock:
    """A block in the Pulse chain."""
    index: int
    timestamp: float
    previous_hash: str
    heartbeats: List[dict]
    transactions: List[dict]
    block_hash: str = ""
    
    # Metrics
    n_live: int = 0           # Number of verified heartbeats
    total_weight: float = 0.0 # Sum of W_i
    security: float = 0.0     # S = Σ W_i
    
    def compute_hash(self) -> str:
        data = {
            "index": self.index,
            "timestamp": self.timestamp,
            "previous_hash": self.previous_hash,
            "heartbeats": self.heartbeats,
            "transactions": self.transactions,
            "n_live": self.n_live,
            "total_weight": self.total_weight
        }
        return hash_block(data)


# =============================================================================
# DEVICE SIMULATOR
# =============================================================================

class Device:
    """Simulates a wearable device capturing heartbeats."""
    
    def __init__(self, name: str = "device"):
        self.name = name
        self.private_key, self.public_key = generate_keypair()
        self.last_heartbeat_time = 0
        
    def capture_heartbeat(self, 
                          hr_base: int = 70, 
                          activity_level: float = 0.0) -> Heartbeat:
        """
        Capture a simulated heartbeat.
        activity_level: 0.0 (resting) to 1.0 (intense exercise)
        """
        # Simulate realistic biometrics
        hr = hr_base + int(activity_level * 60) + random.randint(-5, 5)
        motion = {
            "x": random.gauss(0, 0.1) + activity_level * 0.5,
            "y": random.gauss(0, 0.1) + activity_level * 0.3,
            "z": random.gauss(0, 0.05) + activity_level * 0.2
        }
        temp = 36.5 + random.gauss(0, 0.3) + activity_level * 0.5
        
        hb = Heartbeat(
            timestamp=time.time(),
            heart_rate=hr,
            motion=motion,
            temperature=round(temp, 1),
            device_pubkey=self.public_key
        )
        
        # Sign the heartbeat
        hb.signature = sign_data(self.private_key, hb.to_signable())
        self.last_heartbeat_time = hb.timestamp
        
        return hb


# =============================================================================
# NODE
# =============================================================================

class PulseNode:
    """A node in the Pulse Network."""
    
    def __init__(self, 
                 node_id: str = "genesis",
                 n_threshold: int = 1,
                 block_interval: float = 5.0,
                 reward_per_block: float = 100.0):
        self.node_id = node_id
        self.n_threshold = n_threshold
        self.block_interval = block_interval
        self.reward_per_block = reward_per_block
        
        # State
        self.chain: List[PulseBlock] = []
        self.heartbeat_pool: List[Heartbeat] = []
        self.tx_pool: List[Transaction] = []
        self.balances: Dict[str, float] = {}  # pubkey -> balance
        
        # Metrics
        self.total_minted = 0.0
        self.blocks_created = 0
        
        # Create genesis block
        self._create_genesis()
    
    def _create_genesis(self):
        """Create the genesis block."""
        genesis = PulseBlock(
            index=0,
            timestamp=time.time(),
            previous_hash="0" * 64,
            heartbeats=[],
            transactions=[],
            n_live=0,
            total_weight=0.0,
            security=0.0
        )
        genesis.block_hash = genesis.compute_hash()
        self.chain.append(genesis)
        print(f"🌱 Genesis block created: {genesis.block_hash[:16]}...")
    
    def verify_heartbeat(self, hb: Heartbeat) -> bool:
        """Verify a heartbeat packet."""
        # 1. Check signature
        if not verify_signature(hb.device_pubkey, hb.to_signable(), hb.signature):
            print(f"❌ Invalid signature for heartbeat from {hb.device_pubkey[:8]}...")
            return False
        
        # 2. Check timestamp freshness (within last 30 seconds)
        if time.time() - hb.timestamp > 30:
            print(f"❌ Stale heartbeat from {hb.device_pubkey[:8]}...")
            return False
        
        # 3. Basic sanity checks
        if not (30 <= hb.heart_rate <= 220):
            print(f"❌ Invalid heart rate: {hb.heart_rate}")
            return False
        
        return True
    
    def receive_heartbeat(self, hb: Heartbeat) -> bool:
        """Receive and validate a heartbeat."""
        if self.verify_heartbeat(hb):
            # Check for duplicate in current pool
            for existing in self.heartbeat_pool:
                if existing.device_pubkey == hb.device_pubkey:
                    # Update with newer heartbeat
                    self.heartbeat_pool.remove(existing)
                    break
            
            self.heartbeat_pool.append(hb)
            return True
        return False
    
    def verify_transaction(self, tx: Transaction) -> bool:
        """Verify a transaction."""
        # 1. Check signature
        if not verify_signature(tx.sender_pubkey, tx.to_signable(), tx.signature):
            return False
        
        # 2. Check sender has balance
        sender_balance = self.balances.get(tx.sender_pubkey, 0)
        if sender_balance < tx.amount:
            return False
        
        # 3. Check heartbeat signature exists in pool (sender is alive)
        alive = False
        for hb in self.heartbeat_pool:
            if hb.device_pubkey == tx.sender_pubkey:
                alive = True
                break
        
        if not alive:
            print(f"❌ Transaction rejected: sender not pulsing")
            return False
        
        return True
    
    def receive_transaction(self, tx: Transaction) -> bool:
        """Receive and validate a transaction."""
        if self.verify_transaction(tx):
            self.tx_pool.append(tx)
            return True
        return False
    
    def assemble_block(self) -> Optional[PulseBlock]:
        """Assemble a new Pulse Block from the pool."""
        # Check if we have enough live participants
        n_live = len(self.heartbeat_pool)
        
        if n_live < self.n_threshold:
            print(f"⏳ Waiting for more heartbeats ({n_live}/{self.n_threshold})")
            return None
        
        # Calculate total weight and security
        total_weight = sum(hb.weight() for hb in self.heartbeat_pool)
        security = total_weight  # S = Σ W_i
        
        # Calculate fork probability (for display)
        k = 0.5  # Adjusted for small-scale testing
        fork_prob = math.exp(-k * security)
        
        # Create block
        block = PulseBlock(
            index=len(self.chain),
            timestamp=time.time(),
            previous_hash=self.chain[-1].block_hash,
            heartbeats=[asdict(hb) for hb in self.heartbeat_pool],
            transactions=[asdict(tx) for tx in self.tx_pool],
            n_live=n_live,
            total_weight=round(total_weight, 4),
            security=round(security, 4)
        )
        block.block_hash = block.compute_hash()
        
        print(f"\n💓 PULSE BLOCK #{block.index}")
        print(f"   Hash: {block.block_hash[:16]}...")
        print(f"   Live participants: {n_live}")
        print(f"   Total weight: {total_weight:.4f}")
        print(f"   Security (S): {security:.4f}")
        print(f"   Fork probability: {fork_prob:.6f}")
        
        return block
    
    def commit_block(self, block: PulseBlock):
        """Commit a block and distribute rewards."""
        # Add to chain
        self.chain.append(block)
        self.blocks_created += 1
        
        # Distribute rewards based on weight
        if block.total_weight > 0:
            for hb_dict in block.heartbeats:
                hb = Heartbeat(**hb_dict)
                reward = (hb.weight() / block.total_weight) * self.reward_per_block
                
                pubkey = hb.device_pubkey
                self.balances[pubkey] = self.balances.get(pubkey, 0) + reward
                self.total_minted += reward
                
                print(f"   💰 {pubkey[:8]}... earned {reward:.4f} PULSE")
        
        # Process transactions
        for tx_dict in block.transactions:
            tx = Transaction(**tx_dict)
            self.balances[tx.sender_pubkey] -= tx.amount
            self.balances[tx.recipient_pubkey] = \
                self.balances.get(tx.recipient_pubkey, 0) + tx.amount
            print(f"   📤 TX: {tx.sender_pubkey[:8]}... → {tx.recipient_pubkey[:8]}... ({tx.amount} PULSE)")
        
        # Clear pools
        self.heartbeat_pool = []
        self.tx_pool = []
    
    def get_stats(self) -> dict:
        """Get network statistics."""
        return {
            "chain_length": len(self.chain),
            "total_minted": round(self.total_minted, 4),
            "active_accounts": len(self.balances),
            "balances": {k[:8] + "...": round(v, 4) for k, v in self.balances.items()}
        }


# =============================================================================
# SIMULATION
# =============================================================================

def run_simulation(n_users: int = 3, n_blocks: int = 5):
    """Run a full Pulse Network simulation."""
    
    print("=" * 60)
    print("🫀 PULSE NETWORK - Proof of Life Consensus Prototype")
    print("=" * 60)
    
    # Create node
    node = PulseNode(
        node_id="genesis",
        n_threshold=1,  # Start solo
        block_interval=2.0,
        reward_per_block=100.0
    )
    
    # Create devices (users)
    devices = [Device(name=f"user_{i}") for i in range(n_users)]
    print(f"\n👥 Created {n_users} devices:")
    for d in devices:
        print(f"   • {d.name}: {d.public_key[:16]}...")
    
    # Simulate blocks
    print(f"\n📦 Simulating {n_blocks} blocks...\n")
    
    for block_num in range(n_blocks):
        print(f"\n--- Block interval {block_num + 1} ---")
        
        # Each device sends a heartbeat
        for i, device in enumerate(devices):
            # Vary activity levels
            activity = random.uniform(0, 0.5) if i > 0 else 0.3
            hb = device.capture_heartbeat(activity_level=activity)
            
            if node.receive_heartbeat(hb):
                print(f"✅ {device.name} pulsed (HR: {hb.heart_rate}, W: {hb.weight():.3f})")
        
        # Try to create a block
        block = node.assemble_block()
        if block:
            node.commit_block(block)
        
        # Simulate a transaction after first block
        if block_num == 2 and len(devices) > 1:
            sender = devices[0]
            recipient = devices[1]
            
            # Create transaction
            tx = Transaction(
                tx_id=f"tx_{int(time.time())}",
                sender_pubkey=sender.public_key,
                recipient_pubkey=recipient.public_key,
                amount=10.0,
                timestamp=time.time(),
                heartbeat_signature=node.chain[-1].heartbeats[0]["signature"]
            )
            tx.signature = sign_data(sender.private_key, tx.to_signable())
            
            if node.receive_transaction(tx):
                print(f"\n📨 Transaction queued: {sender.name} → {recipient.name} (10 PULSE)")
        
        time.sleep(0.5)  # Simulate interval
    
    # Final stats
    print("\n" + "=" * 60)
    print("📊 FINAL NETWORK STATE")
    print("=" * 60)
    stats = node.get_stats()
    print(f"Chain length: {stats['chain_length']} blocks")
    print(f"Total minted: {stats['total_minted']} PULSE")
    print(f"Active accounts: {stats['active_accounts']}")
    print("\n💰 Balances:")
    for addr, balance in stats['balances'].items():
        print(f"   {addr}: {balance} PULSE")
    
    # Performance projection
    print("\n📈 PROJECTED PERFORMANCE (at scale):")
    n_global = 100_000_000
    q = 0.1
    n_active = n_global * (1 - q)
    security = n_active  # W_avg = 1
    fork_prob = math.exp(-0.000001 * security)
    tps = 0.1 * n_active / 5
    
    print(f"   At {n_global:,} users (10% offline):")
    print(f"   • Active: {int(n_active):,}")
    print(f"   • Security (S): {security:,.0f}")
    print(f"   • Fork probability: {fork_prob:.2e}")
    print(f"   • Theoretical TPS: {tps:,.0f}")
    
    return node


if __name__ == "__main__":
    run_simulation(n_users=3, n_blocks=5)
