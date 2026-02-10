# Pulse Simulator (Desktop App)

Desktop app to connect to a Pulse node, view live stats and chain data, manage simulator identities, and send heartbeats and transactions.

## Run (web)

```bash
npm install
npm run dev
```

Open http://localhost:5173 and enter your node URL (e.g. `http://localhost:8080` or `http://18.117.9.159:8080`).

## Run (Electron)

```bash
npm run electron:dev
```

Starts Vite and Electron; the app loads in a desktop window.

## Build

```bash
npm run build
```

Output is in `dist/`. For a packaged Electron app, use `npm run electron:build` (requires electron-builder config).

## Features

- **Connect**: Set node URL; health check on connect. URL is persisted.
- **Dashboard**: Health, network stats, chain info, latest block (auto-refresh every 10s).
- **Accounts**: Add simulator identities (secp256k1 keypairs stored locally). View balance per account. Optional "Network accounts" list when the node exposes `GET /accounts`.
- **Simulate**: Send heartbeat (choose account, set heart rate/motion/temp) and send transaction (sender, recipient, amount; sender must have sent a heartbeat first).
