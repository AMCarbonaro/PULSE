export default function Whitepaper() {
  return (
    <div className="whitepaper">
      <h3 className="whitepaper-title">Pulse Network Whitepaper</h3>
      <p className="whitepaper-subtitle">
        A Living Consensus System for a Global Human-Backed Economy
      </p>

      <div className="whitepaper-section">
        <h4 className="whitepaper-heading">Abstract</h4>
        <p>
          We propose a decentralized, biologically-anchored system for global transactions, identity verification, and social activity. Unlike traditional blockchains (Proof-of-Work or Proof-of-Stake), the Pulse Network uses <strong>Proof-of-Life (PoL)</strong> as its core consensus engine. Each human contributes verified biometric signals — primarily heartbeat data — to validate transactions, secure the network, and generate a currency tied to human presence. This offers a global economy, real-time activity measurement, and a verifiable ledger of human engagement.
        </p>
      </div>

      <div className="whitepaper-section">
        <h4 className="whitepaper-heading">1. Introduction</h4>
        <p>
          Current monetary and consensus systems depend on centralized control or artificial scarcity (hashing power, financial stake). The Pulse Network introduces <strong>biological scarcity</strong>: the inability to forge human life. By anchoring validity to live heartbeat data, the network establishes a global, trustless economic ecosystem where every transaction and block is backed by living human participants.
        </p>
      </div>

      <div className="whitepaper-section">
        <h4 className="whitepaper-heading">2. System Architecture</h4>
        <p className="whitepaper-subheading">2.1 Device Layer</p>
        <p>
          Each participant uses a device that captures heart rate (HR), motion (x,y,z), temperature, and optional biometrics. Each heartbeat packet is cryptographically signed so the network can verify authenticity and prevent replay attacks.
        </p>
        <p className="whitepaper-subheading">2.2 Node Layer</p>
        <p>
          Nodes validate and propagate heartbeat signals. They verify signatures and timestamps, maintain a mempool of pulse-backed transactions, and aggregate verified heartbeats and transactions into Pulse Blocks. Transactions without a valid, current heartbeat signature are invalid.
        </p>
        <p className="whitepaper-subheading">2.3 Pulse Block &amp; Consensus</p>
        <p>
          A Pulse Block contains verified heartbeats, transactions, previous block hash, and timestamp. Consensus is achieved when a minimum number of verified human participants contribute heartbeats in a block interval. Block accepted if <code>N_live ≥ N_threshold</code>. Optional weighting: contribution W_i = α·HR_i + β·‖M_i‖ + γ·Δt_i so that activity and continuity increase influence.
        </p>
      </div>

      <div className="whitepaper-section">
        <h4 className="whitepaper-heading">3. Reward Distribution &amp; Token Economics</h4>
        <p>
          Each verified heartbeat earns Pulse tokens. Reward for participant i is proportional to weighted contribution: <code>R_i = (W_i / Σ W_j) · R_total</code>. Supply grows with human participation but is biologically constrained — no synthetic or bot-driven inflation. Inflation π(t) is naturally bounded by block interval and total supply; velocity of money V = Σ TX / T_supply reflects human-backed liquidity.
        </p>
      </div>

      <div className="whitepaper-section">
        <h4 className="whitepaper-heading">4. Security</h4>
        <ul>
          <li><strong>Sybil resistance:</strong> Life cannot be forged; synthetic nodes cannot produce valid heartbeats.</li>
          <li><strong>Replay prevention:</strong> Timestamp and continuity validation reject old or replayed data.</li>
          <li><strong>Fork resolution:</strong> Competing blocks are resolved by cumulative verified life (heaviest chain wins).</li>
          <li><strong>Fork probability:</strong> P_fork = e^(-k·S) — security S is sum of weighted lives; more participants → negligible fork risk.</li>
        </ul>
      </div>

      <div className="whitepaper-section">
        <h4 className="whitepaper-heading">5. Data Flow (End-to-End)</h4>
        <p>
          (1) Device captures and signs heartbeat → (2) Broadcast to nodes; nodes verify signature and freshness → (3) Transactions collected with pulse signatures → (4) Node assembles Pulse Block (heartbeats + transactions) → (5) Proof-of-Life check: minimum live threshold met → (6) Block committed to chain, state updated → (7) Block broadcast to peers → (8) Rewards distributed to live participants → (9) Persistent, immutable ledger of verified human presence and activity.
        </p>
      </div>

      <div className="whitepaper-section">
        <h4 className="whitepaper-heading">6. Applications</h4>
        <p>
          Global economic layer (life-backed currency, micro-payments); identity and reputation verification; gaming and social (activity points, anti-bot); aggregated anonymized health metrics. The ledger is a foundation for Web4 applications where living humans are the source of truth.
        </p>
      </div>

      <div className="whitepaper-section">
        <h4 className="whitepaper-heading">7. Quantitative Properties (Summary)</h4>
        <p>
          Reward per human scales with activity; block security S = Σ W_i; TPS scales with population (λ · N_live); inflation is human-constrained; biometric variability provides entropy for randomness. At scale, the network achieves high throughput and sub-minute finality with negligible fork probability.
        </p>
      </div>

      <div className="whitepaper-section">
        <h4 className="whitepaper-heading">Conclusion</h4>
        <p>
          The Pulse Network replaces energy-intensive or stake-based consensus with a biologically anchored, human-verified ledger. By tying currency and security to actual human life, it establishes a globally resilient economic layer, a new method for identity verification, and a foundation for the next generation of human-centric applications.
        </p>
      </div>
    </div>
  );
}
