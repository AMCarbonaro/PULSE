import type { SimIdentity } from './crypto';

const ACCOUNTS_KEY = 'pulse-sim-accounts';

export type StoredAccount = SimIdentity & { label: string };

export function loadAccounts(): StoredAccount[] {
  try {
    const raw = localStorage.getItem(ACCOUNTS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

export function saveAccounts(accounts: StoredAccount[]): void {
  localStorage.setItem(ACCOUNTS_KEY, JSON.stringify(accounts));
}

export function addAccount(account: StoredAccount): void {
  const accounts = loadAccounts();
  if (accounts.some((a) => a.publicKeyHex === account.publicKeyHex)) return;
  accounts.push(account);
  saveAccounts(accounts);
}

export function removeAccount(publicKeyHex: string): void {
  saveAccounts(loadAccounts().filter((a) => a.publicKeyHex !== publicKeyHex));
}
