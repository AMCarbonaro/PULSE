import { useState, useEffect, useCallback, useRef } from 'react';
import { health, getStats, getChain, getLatestBlock, type NetworkStats, type ChainInfo, type PulseBlock } from '../api';
import { useWebSocket, type WsEvent } from '../useWebSocket';
import Card from './Card';

export default function Dashboard({ nodeUrl }: { nodeUrl: string }) {
  const [healthMsg, setHealthMsg] = useState<string | null>(null);
  const [stats, setStats] = useState<NetworkStats | null>(null);
  const [chain, setChain] = useState<ChainInfo | null>(null);
  const [block, setBlock] = useState<PulseBlock | null>(null);
  const [heartbeatPoolSize, setHeartbeatPoolSize] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [blocksPerSecond, setBlocksPerSecond] = useState<string>('—');
  const blockTimestamps = useRef<number[]>([]);

  const { status: wsStatus } = useWebSocket({
    nodeUrl,
    onEvent: useCallback((event: WsEvent) => {
      switch (event.type) {
        case 'new_block':
          setBlock(event.block);
          const now = Date.now();
          blockTimestamps.current.push(now);
          if (blockTimestamps.current.length > 20) blockTimestamps.current.shift();
          if (blockTimestamps.current.length > 1) {
            const ts = blockTimestamps.current;
            const elapsed = (ts[ts.length - 1] - ts[0]) / 1000;
            const rate = (ts.length - 1) / elapsed;
            setBlocksPerSecond(rate.toFixed(2));
          }
          break;
        case 'stats':
          setStats(event.stats);
          break;
        case 'heartbeat_count':
          setHeartbeatPoolSize(event.count);
          break;
      }
    }, []),
  });

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
    const id = setInterval(() => {
      if (wsStatus !== 'connected') refresh();
    }, 15000);
    return () => clearInterval(id);
  }, [nodeUrl, wsStatus]);

  if (loading && !stats) {
    return (
      <section>
        <h2>Dashboard</h2>
        <p className="text-muted">Loading…</p>
      </section>
    );
  }

  return (
    <section>
      <h2 className="section-title">Dashboard</h2>
      {error && <p className="error-msg">{error}</p>}

      <Card title="Health">
        <div className="health-row">
          <p>{healthMsg ?? '—'}</p>
          <span className={`ws-badge ${wsStatus === 'connected' ? 'ws-badge--connected' : 'ws-badge--disconnected'}`}>
            <span className={`ws-dot ${wsStatus === 'connected' ? 'ws-dot--connected' : 'ws-dot--disconnected'}`} />
            {wsStatus === 'connected' ? 'Live' : wsStatus === 'connecting' ? 'Connecting…' : 'Polling'}
          </span>
        </div>
      </Card>

      <Card title="Network stats">
        <dl className="stats-grid">
          {stats && (
            <>
              <dt>Chain length</dt><dd>{stats.chain_length}</dd>
              <dt>Total minted</dt><dd>{stats.total_minted.toFixed(4)} PULSE</dd>
              <dt>Active accounts</dt><dd>{stats.active_accounts}</dd>
              <dt>Current TPS</dt><dd>{stats.current_tps}</dd>
              <dt>Avg block time</dt><dd>{stats.avg_block_time}s</dd>
              <dt>Total security</dt><dd>{stats.total_security.toFixed(4)}</dd>
            </>
          )}
        </dl>
      </Card>

      <Card title="Chain">
        <dl className="stats-grid">
          {chain && (
            <>
              <dt>Height</dt><dd>{block ? block.index : chain.height}</dd>
              <dt>Latest hash</dt><dd className="hash-text">{block ? block.block_hash : chain.latest_hash || '—'}</dd>
              <dt>Heartbeat pool</dt><dd>{heartbeatPoolSize || chain.heartbeat_pool_size}</dd>
              <dt>Block rate</dt><dd>{blocksPerSecond} blocks/s</dd>
            </>
          )}
        </dl>
      </Card>

      <Card title="Latest block">
        <div>
          {block ? (
            <>
              <p style={{ margin: '0 0 8px 0' }}>Block #{block.index} · hash: <code className="hash-text">{block.block_hash}</code></p>
              <p className="block-summary-sub">Heartbeats: {block.heartbeats.length} · Transactions: {block.transactions.length} · n_live: {block.n_live} · weight: {block.total_weight.toFixed(4)}</p>
            </>
          ) : (
            <p className="text-muted" style={{ margin: 0 }}>No blocks yet</p>
          )}
        </div>
      </Card>
    </section>
  );
}
