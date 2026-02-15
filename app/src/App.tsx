import { useState, useEffect } from 'react';
import './App.css';
import Navbar from './components/Navbar';
import Dashboard from './components/Dashboard';
import ChainExplorer from './components/ChainExplorer';
import AccountsPanel from './components/AccountsPanel';
import Whitepaper from './components/Whitepaper';
import type { Page } from './components/types';

const NODE_URL = 'https://pulse.carbonaromedia.com';

export default function App() {
  const [connected, setConnected] = useState(false);
  const [page, setPage] = useState<Page>('dashboard');
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(true);

  useEffect(() => {
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
      <div className="app-container">
        <h1 className="app-title">Pulse Network</h1>
        <p className="text-muted">Connecting to node…</p>
      </div>
    );
  }

  if (!connected) {
    return (
      <div className="app-container">
        <h1 className="app-title">Pulse Network</h1>
        <p className="text-error">Failed to connect to node: {connectionError}</p>
        <button onClick={() => window.location.reload()} className="retry-btn">
          Retry
        </button>
        <Whitepaper />
      </div>
    );
  }

  return (
    <div className="app-container">
      <h1 className="app-title">Pulse Network</h1>
      <Navbar page={page} setPage={setPage} />
      {page === 'dashboard' && <Dashboard nodeUrl={NODE_URL} />}
      {page === 'chain' && <ChainExplorer nodeUrl={NODE_URL} />}
      {page === 'accounts' && <AccountsPanel nodeUrl={NODE_URL} />}
      {page === 'whitepaper' && <Whitepaper />}
    </div>
  );
}
