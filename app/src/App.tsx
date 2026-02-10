import { useState, useEffect, useCallback } from 'react';
import { health, getStats, getChain, getLatestBlock, getBlocks, getBalance, getAccounts, submitHeartbeat, submitTransaction, type NetworkStats, type ChainInfo, type PulseBlock } from './api';
import { generateKeypair, sign, heartbeatSignablePayload, transactionSignablePayload } from './crypto';
import { loadAccounts, addAccount, removeAccount, type StoredAccount } from './storage';

const NODE_URL = 'https://topics-besides-index-portsmouth.trycloudflare.com';

type Page = 'dashboard' | 'chain' | 'accounts';

export default function App() {
  const [connected, setConnected] = useState(false);
  const [page, setPage] = useState<Page>('dashboard');
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(true);

  useEffect(() => {
    // Auto-connect on load
    (async () => {
      try {
        const res = await fetch(`${NODE_URL}/health`, { signal: AbortSignal.timeout(5000) });
        const data = await res.json();
        if (res.ok && data.success) {
          setConnected(true);
        } else {
          setConnectionError('Node did not return success');
        }
      } catch (e) {
        const msg = e instanceof Error ? e.message : 'Connection failed';
        setConnectionError(msg);
      } finally {
        setConnecting(false);
      }
    })();
  }, []);

  if (connecting) {
    return (
      <div style={{ padding: 24, maxWidth: 900, margin: '0 auto' }}>
        <h1 style={{ marginTop: 0, fontWeight: 600 }}>Pulse Network</h1>
        <p style={{ color: '#a1a1aa' }}>Connecting to node…</p>
      </div>
    );
  }

  if (!connected) {
    return (
      <div style={{ padding: 24, maxWidth: 900, margin: '0 auto' }}>
        <h1 style={{ marginTop: 0, fontWeight: 600 }}>Pulse Network</h1>
        <p style={{ color: '#f87171' }}>Failed to connect to node: {connectionError}</p>
        <button
          onClick={() => window.location.reload()}
          style={{ padding: '8px 16px', borderRadius: 6, border: 'none', background: '#22c55e', color: '#fff', fontWeight: 500 }}
        >
          Retry
        </button>
        <Whitepaper />
      </div>
    );
  }

  return (
    <div style={{ padding: 24, maxWidth: 900, margin: '0 auto' }}>
      <h1 style={{ marginTop: 0, fontWeight: 600 }}>Pulse Network</h1>

      <nav style={{ display: 'flex', gap: 16, marginBottom: 24, flexWrap: 'wrap' }}>
        <button
          onClick={() => setPage('dashboard')}
          style={{ padding: '6px 12px', borderRadius: 6, border: '1px solid #3f3f46', background: page === 'dashboard' ? '#3f3f46' : '#18181b', color: '#e4e4e7' }}
        >
          Dashboard
        </button>
        <button
          onClick={() => setPage('chain')}
          style={{ padding: '6px 12px', borderRadius: 6, border: '1px solid #3f3f46', background: page === 'chain' ? '#3f3f46' : '#18181b', color: '#e4e4e7' }}
        >
          Chain
        </button>
        <button
          onClick={() => setPage('accounts')}
          style={{ padding: '6px 12px', borderRadius: 6, border: '1px solid #3f3f46', background: page === 'accounts' ? '#3f3f46' : '#18181b', color: '#e4e4e7' }}
        >
          Accounts
        </button>
      </nav>

      {page === 'dashboard' && <Dashboard nodeUrl={NODE_URL} />}
      {page === 'chain' && <ChainView nodeUrl={NODE_URL} />}
      {page === 'accounts' && <Accounts nodeUrl={NODE_URL} />}
    </div>
  );
}

function Whitepaper() {
  const sectionStyle = { marginTop: 24, marginBottom: 16 };
  const headingStyle = { fontSize: 16, color: '#e4e4e7', marginBottom: 8 };
  const subStyle = { fontSize: 14, color: '#a1a1aa', marginBottom: 6 };
  const pStyle = { margin: '0 0 12px 0', fontSize: 13, lineHeight: 1.6, color: '#d4d4d8' };
  const codeStyle = { fontFamily: 'monospace', fontSize: 12, background: '#27272a', padding: '2px 6px', borderRadius: 4, color: '#a1a1aa' };
  const ulStyle = { margin: '0 0 12px 0', paddingLeft: 20, fontSize: 13, lineHeight: 1.6, color: '#d4d4d8' };
  return (
    <div style={{ marginTop: 32, maxHeight: '60vh', overflowY: 'auto', paddingRight: 8 }}>
      <h3 style={{ margin: '0 0 8px 0', fontSize: 18, color: '#e4e4e7' }}>Pulse Network Whitepaper</h3>
      <p style={{ ...pStyle, color: '#a1a1aa', fontStyle: 'italic' }}>
        A Living Consensus System for a Global Human-Backed Economy
      </p>

      <div style={sectionStyle}>
        <h4 style={headingStyle}>Abstract</h4>
        <p style={pStyle}>
          We propose a decentralized, biologically-anchored system for global transactions, identity verification, and social activity. Unlike traditional blockchains (Proof-of-Work or Proof-of-Stake), the Pulse Network uses <strong>Proof-of-Life (PoL)</strong> as its core consensus engine. Each human contributes verified biometric signals — primarily heartbeat data — to validate transactions, secure the network, and generate a currency tied to human presence. This offers a global economy, real-time activity measurement, and a verifiable ledger of human engagement.
        </p>
      </div>

      <div style={sectionStyle}>
        <h4 style={headingStyle}>1. Introduction</h4>
        <p style={pStyle}>
          Current monetary and consensus systems depend on centralized control or artificial scarcity (hashing power, financial stake). The Pulse Network introduces <strong>biological scarcity</strong>: the inability to forge human life. By anchoring validity to live heartbeat data, the network establishes a global, trustless economic ecosystem where every transaction and block is backed by living human participants.
        </p>
      </div>

      <div style={sectionStyle}>
        <h4 style={headingStyle}>2. System Architecture</h4>
        <p style={subStyle}>2.1 Device Layer</p>
        <p style={pStyle}>
          Each participant uses a device that captures heart rate (HR), motion (x,y,z), temperature, and optional biometrics. Each heartbeat packet is cryptographically signed so the network can verify authenticity and prevent replay attacks.
        </p>
        <p style={subStyle}>2.2 Node Layer</p>
        <p style={pStyle}>
          Nodes validate and propagate heartbeat signals. They verify signatures and timestamps, maintain a mempool of pulse-backed transactions, and aggregate verified heartbeats and transactions into Pulse Blocks. Transactions without a valid, current heartbeat signature are invalid.
        </p>
        <p style={subStyle}>2.3 Pulse Block &amp; Consensus</p>
        <p style={pStyle}>
          A Pulse Block contains verified heartbeats, transactions, previous block hash, and timestamp. Consensus is achieved when a minimum number of verified human participants contribute heartbeats in a block interval. Block accepted if <span style={codeStyle}>N_live ≥ N_threshold</span>. Optional weighting: contribution W_i = α·HR_i + β·‖M_i‖ + γ·Δt_i so that activity and continuity increase influence.
        </p>
      </div>

      <div style={sectionStyle}>
        <h4 style={headingStyle}>3. Reward Distribution &amp; Token Economics</h4>
        <p style={pStyle}>
          Each verified heartbeat earns Pulse tokens. Reward for participant i is proportional to weighted contribution: <span style={codeStyle}>R_i = (W_i / Σ W_j) · R_total</span>. Supply grows with human participation but is biologically constrained — no synthetic or bot-driven inflation. Inflation π(t) is naturally bounded by block interval and total supply; velocity of money V = Σ TX / T_supply reflects human-backed liquidity.
        </p>
      </div>

      <div style={sectionStyle}>
        <h4 style={headingStyle}>4. Security</h4>
        <ul style={ulStyle}>
          <li><strong>Sybil resistance:</strong> Life cannot be forged; synthetic nodes cannot produce valid heartbeats.</li>
          <li><strong>Replay prevention:</strong> Timestamp and continuity validation reject old or replayed data.</li>
          <li><strong>Fork resolution:</strong> Competing blocks are resolved by cumulative verified life (heaviest chain wins).</li>
          <li><strong>Fork probability:</strong> P_fork = e^(-k·S) — security S is sum of weighted lives; more participants → negligible fork risk.</li>
        </ul>
      </div>

      <div style={sectionStyle}>
        <h4 style={headingStyle}>5. Data Flow (End-to-End)</h4>
        <p style={pStyle}>
          (1) Device captures and signs heartbeat → (2) Broadcast to nodes; nodes verify signature and freshness → (3) Transactions collected with pulse signatures → (4) Node assembles Pulse Block (heartbeats + transactions) → (5) Proof-of-Life check: minimum live threshold met → (6) Block committed to chain, state updated → (7) Block broadcast to peers → (8) Rewards distributed to live participants → (9) Persistent, immutable ledger of verified human presence and activity.
        </p>
      </div>

      <div style={sectionStyle}>
        <h4 style={headingStyle}>6. Applications</h4>
        <p style={pStyle}>
          Global economic layer (life-backed currency, micro-payments); identity and reputation verification; gaming and social (activity points, anti-bot); aggregated anonymized health metrics. The ledger is a foundation for Web4 applications where living humans are the source of truth.
        </p>
      </div>

      <div style={sectionStyle}>
        <h4 style={headingStyle}>7. Quantitative Properties (Summary)</h4>
        <p style={pStyle}>
          Reward per human scales with activity; block security S = Σ W_i; TPS scales with population (λ · N_live); inflation is human-constrained; biometric variability provides entropy for randomness. At scale, the network achieves high throughput and sub-minute finality with negligible fork probability.
        </p>
      </div>

      <div style={sectionStyle}>
        <h4 style={headingStyle}>Conclusion</h4>
        <p style={pStyle}>
          The Pulse Network replaces energy-intensive or stake-based consensus with a biologically anchored, human-verified ledger. By tying currency and security to actual human life, it establishes a globally resilient economic layer, a new method for identity verification, and a foundation for the next generation of human-centric applications.
        </p>
      </div>
    </div>
  );
}

function Dashboard({ nodeUrl }: { nodeUrl: string }) {
  const [healthMsg, setHealthMsg] = useState<string | null>(null);
  const [stats, setStats] = useState<NetworkStats | null>(null);
  const [chain, setChain] = useState<ChainInfo | null>(null);
  const [block, setBlock] = useState<PulseBlock | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    setLoading(true);
    setError(null);
    try {
      const [h, s, c, b] = await Promise.all([
        health(nodeUrl),
        getStats(nodeUrl),
        getChain(nodeUrl),
        getLatestBlock(nodeUrl),
      ]);
      setHealthMsg(h.success ? (h.data ?? 'OK') : h.error ?? 'Failed');
      setStats(s.success ? s.data ?? null : null);
      setChain(c.success ? (c.data ?? null) : null);
      setBlock(b.success ? (b.data ?? null) : null);
      if (!h.success) setError('Health check failed');
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Fetch failed');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 5000);
    return () => clearInterval(id);
  }, [nodeUrl]);

  const card = (title: string, children: React.ReactNode) => (
    <div style={{ background: '#18181b', borderRadius: 8, padding: 16, marginBottom: 16 }}>
      <h3 style={{ margin: '0 0 12px 0', fontSize: 14, color: '#a1a1aa' }}>{title}</h3>
      {children}
    </div>
  );

  if (loading && !stats) {
    return (
      <section>
        <h2>Dashboard</h2>
        <p style={{ color: '#a1a1aa' }}>Loading…</p>
      </section>
    );
  }

  return (
    <section>
      <h2 style={{ margin: 0 }}>Dashboard</h2>
      {error && <p style={{ color: '#f87171', marginTop: 8 }}>{error}</p>}

      {card('Health', <p style={{ margin: 0 }}>{healthMsg ?? '—'}</p>)}

      {card('Network stats', (
        <dl style={{ margin: 0, display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '4px 16px' }}>
          {stats && (
            <>
              <dt style={{ color: '#a1a1aa' }}>Chain length</dt><dd>{stats.chain_length}</dd>
              <dt style={{ color: '#a1a1aa' }}>Total minted</dt><dd>{stats.total_minted.toFixed(4)} PULSE</dd>
              <dt style={{ color: '#a1a1aa' }}>Active accounts</dt><dd>{stats.active_accounts}</dd>
              <dt style={{ color: '#a1a1aa' }}>Current TPS</dt><dd>{stats.current_tps}</dd>
              <dt style={{ color: '#a1a1aa' }}>Avg block time</dt><dd>{stats.avg_block_time}s</dd>
              <dt style={{ color: '#a1a1aa' }}>Total security</dt><dd>{stats.total_security.toFixed(4)}</dd>
            </>
          )}
        </dl>
      ))}

      {card('Chain', (
        <dl style={{ margin: 0, display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '4px 16px' }}>
          {chain && (
            <>
              <dt style={{ color: '#a1a1aa' }}>Height</dt><dd>{chain.height}</dd>
              <dt style={{ color: '#a1a1aa' }}>Latest hash</dt><dd style={{ wordBreak: 'break-all', fontFamily: 'monospace', fontSize: 12 }}>{chain.latest_hash || '—'}</dd>
              <dt style={{ color: '#a1a1aa' }}>Heartbeat pool size</dt><dd>{chain.heartbeat_pool_size}</dd>
            </>
          )}
        </dl>
      ))}

      {card('Latest block', (
        <div>
          {block ? (
            <>
              <p style={{ margin: '0 0 8px 0' }}>Block #{block.index} · hash: <code style={{ fontSize: 12, wordBreak: 'break-all' }}>{block.block_hash}</code></p>
              <p style={{ margin: 0, color: '#a1a1aa', fontSize: 14 }}>Heartbeats: {block.heartbeats.length} · Transactions: {block.transactions.length} · n_live: {block.n_live} · weight: {block.total_weight.toFixed(4)}</p>
            </>
          ) : (
            <p style={{ margin: 0, color: '#a1a1aa' }}>No blocks yet</p>
          )}
        </div>
      ))}
    </section>
  );
}

function ChainView({ nodeUrl }: { nodeUrl: string }) {
  const [blocks, setBlocks] = useState<PulseBlock[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null);
  const [blockSearch, setBlockSearch] = useState('');

  const refresh = useCallback(async () => {
    setError(null);
    try {
      const res = await getBlocks(nodeUrl);
      if (res.success && res.data) setBlocks(res.data);
      else setError(res.error ?? 'Failed to load chain');
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to load chain');
    } finally {
      setLoading(false);
    }
  }, [nodeUrl]);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 5000);
    return () => clearInterval(id);
  }, [refresh]);

  const formatTime = (ms: number) => {
    const d = new Date(ms);
    return d.toLocaleString();
  };

  // Newest first; filter by search (block index or hash)
  const filteredBlocks = blocks
    .slice()
    .reverse()
    .filter((block) => {
      if (!blockSearch.trim()) return true;
      const q = blockSearch.trim().toLowerCase();
      return String(block.index).includes(q) || block.block_hash.toLowerCase().includes(q) || (block.previous_hash && block.previous_hash.toLowerCase().includes(q));
    });

  if (loading && blocks.length === 0) {
    return (
      <section>
        <h2>Chain</h2>
        <p style={{ color: '#a1a1aa' }}>Loading…</p>
      </section>
    );
  }

  return (
    <section>
      <h2 style={{ margin: 0 }}>Chain</h2>
      {error && <p style={{ color: '#f87171', marginTop: 8 }}>{error}</p>}
      {blocks.length === 0 ? (
        <p style={{ color: '#a1a1aa', marginTop: 16 }}>No blocks yet</p>
      ) : (
        <>
          <div style={{ marginTop: 16, display: 'flex', flexWrap: 'wrap', gap: 8, alignItems: 'center' }}>
            <label style={{ color: '#a1a1aa', fontSize: 14 }}>
              Search block:
              <input
                type="text"
                value={blockSearch}
                onChange={(e) => setBlockSearch(e.target.value)}
                placeholder="Index or hash..."
                style={{ marginLeft: 8, padding: '6px 10px', borderRadius: 6, border: '1px solid #3f3f46', background: '#27272a', color: '#e4e4e7', width: 220, fontFamily: 'monospace', fontSize: 12 }}
              />
            </label>
            {blockSearch.trim() && <span style={{ fontSize: 12, color: '#a1a1aa' }}>{filteredBlocks.length} block{filteredBlocks.length !== 1 ? 's' : ''}</span>}
          </div>
          <div style={{ marginTop: 12, maxHeight: '70vh', overflowY: 'auto', paddingRight: 4 }}>
          {filteredBlocks.map((block) => (
            <div key={block.index} style={{ background: '#18181b', borderRadius: 8, padding: 16, marginBottom: 12 }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', flexWrap: 'wrap', gap: 8 }}>
                <div>
                  <strong>Block #{block.index}</strong>
                  <div style={{ marginTop: 6, fontFamily: 'monospace', fontSize: 12, wordBreak: 'break-all', color: '#e4e4e7' }}>
                    Hash: {block.block_hash}
                  </div>
                  <div style={{ marginTop: 4, fontFamily: 'monospace', fontSize: 12, wordBreak: 'break-all', color: '#a1a1aa' }}>
                    Prev: {block.previous_hash || '—'}
                  </div>
                  <div style={{ marginTop: 4, fontSize: 12, color: '#a1a1aa' }}>
                    {formatTime(block.timestamp)} · Heartbeats: {block.heartbeats.length} · Transactions: {block.transactions.length} · n_live: {block.n_live} · weight: {block.total_weight.toFixed(4)} · security: {block.security.toFixed(4)}
                  </div>
                </div>
                <button onClick={() => setExpandedIndex(expandedIndex === block.index ? null : block.index)} style={{ padding: '6px 12px', borderRadius: 6, border: '1px solid #3f3f46', background: '#27272a', color: '#e4e4e7' }}>
                  {expandedIndex === block.index ? 'Hide details' : 'Show details'}
                </button>
              </div>
              {expandedIndex === block.index && (
                <div style={{ marginTop: 16, paddingTop: 16, borderTop: '1px solid #3f3f46' }}>
                  {block.heartbeats.length > 0 && (
                    <div style={{ marginBottom: 16 }}>
                      <h4 style={{ margin: '0 0 8px 0', fontSize: 12, color: '#a1a1aa' }}>Heartbeats</h4>
                      <ul style={{ listStyle: 'none', padding: 0, margin: 0, fontFamily: 'monospace', fontSize: 11 }}>
                        {block.heartbeats.map((hb, i) => (
                          <li key={i} style={{ padding: '6px 0', borderBottom: '1px solid #27272a' }}>
                            {hb.device_pubkey.slice(0, 16)}… · HR: {hb.heart_rate} · motion: ({hb.motion.x.toFixed(2)}, {hb.motion.y.toFixed(2)}, {hb.motion.z.toFixed(2)}) · temp: {hb.temperature}
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}
                  {block.transactions.length > 0 && (
                    <div>
                      <h4 style={{ margin: '0 0 8px 0', fontSize: 12, color: '#a1a1aa' }}>Transactions</h4>
                      <ul style={{ listStyle: 'none', padding: 0, margin: 0, fontFamily: 'monospace', fontSize: 11 }}>
                        {block.transactions.map((tx, i) => (
                          <li key={i} style={{ padding: '6px 0', borderBottom: '1px solid #27272a' }}>
                            {tx.sender_pubkey.slice(0, 12)}… → {tx.recipient_pubkey.slice(0, 12)}… · {tx.amount} PULSE
                          </li>
                        ))}
                      </ul>
                    </div>
                  )}
                  {block.heartbeats.length === 0 && block.transactions.length === 0 && (
                    <p style={{ margin: 0, color: '#a1a1aa', fontSize: 12 }}>No heartbeats or transactions in this block.</p>
                  )}
                </div>
              )}
            </div>
          ))}
          </div>
        </>
      )}
    </section>
  );
}

function Accounts({ nodeUrl }: { nodeUrl: string }) {
  const [accounts, setAccounts] = useState(() => loadAccounts());
  const [balances, setBalances] = useState<Record<string, number>>({});
  const [networkAccounts, setNetworkAccounts] = useState<Array<{ pubkey: string; balance: number }>>([]);
  const [adding, setAdding] = useState(false);

  const refreshBalances = useCallback(async () => {
    const next: Record<string, number> = {};
    for (const a of accounts) {
      const res = await getBalance(nodeUrl, a.publicKeyHex);
      next[a.publicKeyHex] = res.success && res.data ? res.data.balance : 0;
    }
    setBalances((prev) => ({ ...prev, ...next }));
    const accRes = await getAccounts(nodeUrl);
    if (accRes.success && accRes.data) {
      setNetworkAccounts(accRes.data.map((a) => ({ pubkey: a.pubkey, balance: a.balance })));
    }
  }, [nodeUrl, accounts]);

  useEffect(() => {
    refreshBalances();
  }, [accounts.length, nodeUrl, refreshBalances]);

  const handleAdd = async () => {
    setAdding(true);
    try {
      const identity = await generateKeypair();
      const stored: StoredAccount = { ...identity, label: `Account ${accounts.length + 1}` };
      addAccount(stored);
      setAccounts(loadAccounts());
      const res = await getBalance(nodeUrl, stored.publicKeyHex);
      setBalances((prev) => ({ ...prev, [stored.publicKeyHex]: res.success && res.data ? res.data.balance : 0 }));
    } finally {
      setAdding(false);
    }
  };

  const remove = (publicKeyHex: string) => {
    removeAccount(publicKeyHex);
    setAccounts(loadAccounts());
    setBalances((prev) => {
      const next = { ...prev };
      delete next[publicKeyHex];
      return next;
    });
  };

  return (
    <section>
      <h2 style={{ margin: 0 }}>Accounts</h2>
      <p style={{ color: '#a1a1aa', marginTop: 8 }}>Keypairs you control (stored in this browser) are listed first. All accounts on the node with a balance appear in Network accounts below.</p>

      <h3 style={{ fontSize: 14, color: '#a1a1aa', marginTop: 20, marginBottom: 8 }}>Keypairs in this browser</h3>
      <ul style={{ listStyle: 'none', padding: 0, margin: 0 }}>
        {accounts.map((a) => (
          <li key={a.publicKeyHex} style={{ background: '#18181b', borderRadius: 8, padding: 16, marginBottom: 8, display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: 8 }}>
            <div>
              <strong>{a.label}</strong>
              <div style={{ fontFamily: 'monospace', fontSize: 12, wordBreak: 'break-all', color: '#a1a1aa' }}>{a.publicKeyHex}</div>
              <div style={{ marginTop: 4 }}>Balance: <strong>{(balances[a.publicKeyHex] ?? 0).toFixed(4)} PULSE</strong></div>
            </div>
            <button onClick={() => remove(a.publicKeyHex)} style={{ padding: '6px 12px', borderRadius: 6, border: '1px solid #ef4444', background: 'transparent', color: '#ef4444' }}>
              Remove
            </button>
          </li>
        ))}
      </ul>
      {accounts.length === 0 && !adding && (
        <p style={{ color: '#a1a1aa' }}>No keypairs stored in this browser.</p>
      )}

      <div style={{ marginTop: 24 }}>
        <h3 style={{ fontSize: 14, color: '#a1a1aa', marginBottom: 8 }}>Network accounts (all on node)</h3>
        {networkAccounts.length > 0 ? (
          <ul style={{ listStyle: 'none', padding: 0, margin: 0 }}>
            {networkAccounts.map((a) => (
              <li key={a.pubkey} style={{ background: '#18181b', borderRadius: 8, padding: 12, marginBottom: 8, fontFamily: 'monospace', fontSize: 12 }}>
                <span style={{ wordBreak: 'break-all' }}>{a.pubkey}</span>
                <span style={{ marginLeft: 8, color: '#a1a1aa' }}>{a.balance.toFixed(4)} PULSE</span>
              </li>
            ))}
          </ul>
        ) : (
          <p style={{ color: '#a1a1aa' }}>No accounts on the node yet, or still loading.</p>
        )}
      </div>
    </section>
  );
}
