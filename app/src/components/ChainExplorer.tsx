import { useState, useEffect, useCallback } from 'react';
import { getBlocks, type PulseBlock } from '../api';

export default function ChainExplorer({ nodeUrl }: { nodeUrl: string }) {
  const [blocks, setBlocks] = useState<PulseBlock[]>([]);
  const [totalBlocks, setTotalBlocks] = useState(0);
  const [currentPage, setCurrentPage] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null);
  const [blockSearch, setBlockSearch] = useState('');
  const PAGE_SIZE = 50;

  const refresh = useCallback(async () => {
    setError(null);
    try {
      const offset = currentPage > 0 ? (totalBlocks - (currentPage + 1) * PAGE_SIZE) : undefined;
      const res = await getBlocks(nodeUrl, offset !== undefined ? Math.max(0, offset) : undefined, PAGE_SIZE);
      if (res.success && res.data) {
        setBlocks(res.data.blocks);
        setTotalBlocks(res.data.total);
      } else {
        setError(res.error ?? 'Failed to load chain');
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to load chain');
    } finally {
      setLoading(false);
    }
  }, [nodeUrl, currentPage, totalBlocks]);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 10000);
    return () => clearInterval(id);
  }, [refresh]);

  const totalPages = Math.ceil(totalBlocks / PAGE_SIZE);

  const formatTime = (ms: number) => {
    const d = new Date(ms);
    return d.toLocaleString();
  };

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
        <p className="text-muted">Loading…</p>
      </section>
    );
  }

  return (
    <section>
      <h2 className="section-title">Chain</h2>
      {error && <p className="error-msg">{error}</p>}
      {blocks.length === 0 ? (
        <p className="text-muted" style={{ marginTop: 16 }}>No blocks yet</p>
      ) : (
        <>
          <div className="search-bar">
            <label>
              Search block:
              <input
                type="text"
                value={blockSearch}
                onChange={(e) => setBlockSearch(e.target.value)}
                placeholder="Index or hash..."
                className="search-input"
              />
            </label>
            {blockSearch.trim() && <span className="search-count">{filteredBlocks.length} block{filteredBlocks.length !== 1 ? 's' : ''}</span>}
          </div>
          <div className="block-list">
            {filteredBlocks.map((block) => (
              <div key={block.index} className="block-card">
                <div className="block-card-header">
                  <div>
                    <strong>Block #{block.index}</strong>
                    <div className="block-hash">Hash: {block.block_hash}</div>
                    <div className="block-prev-hash">Prev: {block.previous_hash || '—'}</div>
                    <div className="block-meta">
                      {formatTime(block.timestamp)} · Heartbeats: {block.heartbeats.length} · Transactions: {block.transactions.length} · n_live: {block.n_live} · weight: {block.total_weight.toFixed(4)} · security: {block.security.toFixed(4)}
                    </div>
                  </div>
                  <button onClick={() => setExpandedIndex(expandedIndex === block.index ? null : block.index)} className="details-btn">
                    {expandedIndex === block.index ? 'Hide details' : 'Show details'}
                  </button>
                </div>
                {expandedIndex === block.index && (
                  <div className="block-details">
                    {block.heartbeats.length > 0 && (
                      <div className="block-details-section">
                        <h4>Heartbeats</h4>
                        <ul>
                          {block.heartbeats.map((hb, i) => (
                            <li key={i}>
                              {hb.device_pubkey.slice(0, 16)}… · HR: {hb.heart_rate} · motion: ({hb.motion.x.toFixed(2)}, {hb.motion.y.toFixed(2)}, {hb.motion.z.toFixed(2)}) · temp: {hb.temperature}
                            </li>
                          ))}
                        </ul>
                      </div>
                    )}
                    {block.transactions.length > 0 && (
                      <div>
                        <h4>Transactions</h4>
                        <ul>
                          {block.transactions.map((tx, i) => (
                            <li key={i}>
                              {tx.sender_pubkey.slice(0, 12)}… → {tx.recipient_pubkey.slice(0, 12)}… · {tx.amount} PULSE
                            </li>
                          ))}
                        </ul>
                      </div>
                    )}
                    {block.heartbeats.length === 0 && block.transactions.length === 0 && (
                      <p className="text-muted" style={{ margin: 0, fontSize: 12 }}>No heartbeats or transactions in this block.</p>
                    )}
                  </div>
                )}
              </div>
            ))}
          </div>
          {totalPages > 1 && (
            <div className="pagination">
              <button
                onClick={() => setCurrentPage(Math.min(currentPage + 1, totalPages - 1))}
                disabled={currentPage >= totalPages - 1}
                className="pagination-btn"
              >
                ← Older
              </button>
              <span className="pagination-info">
                Page {currentPage + 1} of {totalPages} ({totalBlocks} blocks)
              </span>
              <button
                onClick={() => setCurrentPage(Math.max(currentPage - 1, 0))}
                disabled={currentPage === 0}
                className="pagination-btn"
              >
                Newer →
              </button>
            </div>
          )}
        </>
      )}
    </section>
  );
}
