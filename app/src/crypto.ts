import * as secp from '@noble/secp256k1';

/** secp256k1 keypair for simulator identity; matches node's k256 format */
export type SimIdentity = {
  id: string;
  publicKeyHex: string;
  privateKeyHex: string;
};

/** Generate a new keypair. Public key is SEC1 uncompressed (65 bytes) hex. */
export async function generateKeypair(): Promise<SimIdentity> {
  const privateKey = secp.utils.randomPrivateKey();
  const publicKey = secp.getPublicKey(privateKey, false); // uncompressed
  return {
    id: crypto.randomUUID(),
    publicKeyHex: bytesToHex(publicKey),
    privateKeyHex: bytesToHex(privateKey),
  };
}

/** Restore identity from private key hex (32 bytes) */
export async function identityFromPrivateKey(privateKeyHex: string): Promise<SimIdentity> {
  const privateKey = hexToBytes(privateKeyHex);
  const publicKey = secp.getPublicKey(privateKey, false);
  return {
    id: crypto.randomUUID(),
    publicKeyHex: bytesToHex(publicKey),
    privateKeyHex: privateKeyHex,
  };
}

/** SHA-256 hash (node uses SHA-256 before ECDSA sign) */
async function sha256(data: Uint8Array): Promise<Uint8Array> {
  const hash = await crypto.subtle.digest('SHA-256', data.slice(0));
  return new Uint8Array(hash);
}

/** Sign data and return 64-byte compact signature hex (hashes with SHA-256 first, like k256) */
export async function sign(privateKeyHex: string, data: Uint8Array): Promise<string> {
  const privateKey = hexToBytes(privateKeyHex);
  const hash = await sha256(data);
  const sig = await secp.signAsync(hash, privateKey);
  return bytesToHex(sig.toCompactRawBytes());
}

function bytesToHex(b: Uint8Array): string {
  return Array.from(b)
    .map((x) => x.toString(16).padStart(2, '0'))
    .join('');
}

function hexToBytes(hex: string): Uint8Array {
  const len = hex.length / 2;
  const out = new Uint8Array(len);
  for (let i = 0; i < len; i++) out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  return out;
}

/** Format number like Rust serde_json (floats get .0 so signable bytes match node) */
function rustNum(n: number): string {
  return Number.isInteger(n) ? `${n}.0` : String(n);
}

/** Build signable JSON for heartbeat (byte-exact match with node's signable_bytes) */
export function heartbeatSignablePayload(payload: {
  timestamp: number;
  heart_rate: number;
  motion: { x: number; y: number; z: number };
  temperature: number;
  device_pubkey: string;
}): Uint8Array {
  const { timestamp, heart_rate, motion, temperature, device_pubkey } = payload;
  // Node uses u64/u16 for timestamp/heart_rate (no decimal), f64/f32 for motion and temperature (.0)
  const json = `{"timestamp":${Math.floor(timestamp)},"heart_rate":${Math.floor(heart_rate)},"motion":{"x":${rustNum(motion.x)},"y":${rustNum(motion.y)},"z":${rustNum(motion.z)}},"temperature":${rustNum(temperature)},"device_pubkey":${JSON.stringify(device_pubkey)}}`;
  return new TextEncoder().encode(json);
}

/** Build signable JSON for transaction (field order matches node) */
export function transactionSignablePayload(payload: {
  tx_id: string;
  sender_pubkey: string;
  recipient_pubkey: string;
  amount: number;
  timestamp: number;
  heartbeat_signature: string;
}): Uint8Array {
  const json = JSON.stringify(payload);
  return new TextEncoder().encode(json);
}
