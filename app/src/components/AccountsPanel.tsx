import { useState, useEffect, useCallback } from 'react';
import { getBalance, getAccounts } from '../api';
import { generateKeypair } from '../crypto';
import { loadAccounts, addAccount, removeAccount, type StoredAccount } from '../storage';

export default function AccountsPanel({ nodeUrl }: { nodeUrl: string }) {
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
      <h2 className="section-title">Accounts</h2>
      <p className="accounts-description">Keypairs you control (stored in this browser) are listed first. All accounts on the node with a balance appear in Network accounts below.</p>

      <h3 className="accounts-section-title">Keypairs in this browser</h3>
      <ul className="account-list">
        {accounts.map((a) => (
          <li key={a.publicKeyHex} className="account-item">
            <div>
              <strong>{a.label}</strong>
              <div className="account-pubkey">{a.publicKeyHex}</div>
              <div style={{ marginTop: 4 }}>Balance: <strong>{(balances[a.publicKeyHex] ?? 0).toFixed(4)} PULSE</strong></div>
            </div>
            <button onClick={() => remove(a.publicKeyHex)} className="remove-btn">
              Remove
            </button>
          </li>
        ))}
      </ul>
      {accounts.length === 0 && !adding && (
        <p className="text-muted">No keypairs stored in this browser.</p>
      )}

      <div style={{ marginTop: 24 }}>
        <h3 className="accounts-section-title">Network accounts (all on node)</h3>
        {networkAccounts.length > 0 ? (
          <ul className="account-list">
            {networkAccounts.map((a) => (
              <li key={a.pubkey} className="network-account-item">
                <span>{a.pubkey}</span>
                <span className="network-account-balance">{a.balance.toFixed(4)} PULSE</span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-muted">No accounts on the node yet, or still loading.</p>
        )}
      </div>
    </section>
  );
}
