const base = (nodeUrl: string) => nodeUrl.replace(/\/$/, '');

/** Parse JSON response safely; empty or invalid body returns an error response instead of throwing. */
async function parseJsonResponse<T>(res: Response): Promise<ApiResponse<T>> {
  const text = await res.text();
  if (!text.trim()) {
    return { success: false, error: res.ok ? 'Empty response' : `Request failed (${res.status})` };
  }
  try {
    return JSON.parse(text) as ApiResponse<T>;
  } catch {
    return { success: false, error: 'Invalid JSON response' };
  }
}

export type ApiResponse<T> = { success: boolean; data?: T; error?: string };

export type NetworkStats = {
  chain_length: number;
  total_minted: number;
  active_accounts: number;
  current_tps: number;
  avg_block_time: number;
  total_security: number;
};

export type ChainInfo = {
  height: number;
  latest_hash: string;
  heartbeat_pool_size: number;
};

export type Motion = { x: number; y: number; z: number };

export type Heartbeat = {
  timestamp: number;
  heart_rate: number;
  motion: Motion;
  temperature: number;
  device_pubkey: string;
  signature: string;
};

export type Transaction = {
  tx_id: string;
  sender_pubkey: string;
  recipient_pubkey: string;
  amount: number;
  timestamp: number;
  heartbeat_signature: string;
  signature: string;
};

export type PulseBlock = {
  index: number;
  timestamp: number;
  previous_hash: string;
  heartbeats: Heartbeat[];
  transactions: Transaction[];
  n_live: number;
  total_weight: number;
  security: number;
  block_hash: string;
};

export type Account = {
  pubkey: string;
  balance: number;
  last_heartbeat: number;
  total_earned: number;
  blocks_participated: number;
};

export async function getAccounts(nodeUrl: string): Promise<ApiResponse<Account[]>> {
  const res = await fetch(`${base(nodeUrl)}/accounts`);
  return parseJsonResponse<Account[]>(res);
}

export async function health(nodeUrl: string): Promise<ApiResponse<string>> {
  const res = await fetch(`${base(nodeUrl)}/health`);
  return parseJsonResponse<string>(res);
}

export async function getStats(nodeUrl: string): Promise<ApiResponse<NetworkStats>> {
  const res = await fetch(`${base(nodeUrl)}/stats`);
  return parseJsonResponse<NetworkStats>(res);
}

export async function getChain(nodeUrl: string): Promise<ApiResponse<ChainInfo>> {
  const res = await fetch(`${base(nodeUrl)}/chain`);
  return parseJsonResponse<ChainInfo>(res);
}

export async function getLatestBlock(nodeUrl: string): Promise<ApiResponse<PulseBlock>> {
  const res = await fetch(`${base(nodeUrl)}/block/latest`);
  return parseJsonResponse<PulseBlock>(res);
}

export async function getBlocks(nodeUrl: string): Promise<ApiResponse<PulseBlock[]>> {
  const res = await fetch(`${base(nodeUrl)}/blocks`);
  return parseJsonResponse<PulseBlock[]>(res);
}

export async function getBlockByIndex(nodeUrl: string, index: number): Promise<ApiResponse<PulseBlock>> {
  const res = await fetch(`${base(nodeUrl)}/block/${index}`);
  return parseJsonResponse<PulseBlock>(res);
}

export async function getBalance(nodeUrl: string, pubkey: string): Promise<ApiResponse<{ pubkey: string; balance: number }>> {
  const res = await fetch(`${base(nodeUrl)}/balance/${encodeURIComponent(pubkey)}`);
  return parseJsonResponse<{ pubkey: string; balance: number }>(res);
}

export async function submitHeartbeat(nodeUrl: string, body: Heartbeat): Promise<ApiResponse<unknown>> {
  const res = await fetch(`${base(nodeUrl)}/pulse`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  return parseJsonResponse<unknown>(res);
}

export async function submitTransaction(nodeUrl: string, body: Transaction): Promise<ApiResponse<unknown>> {
  const res = await fetch(`${base(nodeUrl)}/tx`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  return parseJsonResponse<unknown>(res);
}
