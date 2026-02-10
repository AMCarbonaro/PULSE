# Restart the Pulse node on the instance

Use this when you need to restart the node (e.g. after deploying the new binary with `GET /blocks`) so the Chain page stops returning 404.

**SSH target / frontend node URL:** `18.117.9.159` (API on port 8080)

**To use the app on Netlify (HTTPS)** you need the node on HTTPS too. See **[HTTPS-NODE-CLOUDFLARE.md](./HTTPS-NODE-CLOUDFLARE.md)** for exposing the node with a Cloudflare Tunnel.

---

## 1. SSH into the instance

From your Mac:

```bash
ssh -i ~/Desktop/Pulse/infrastructure/pulse-key.pem ec2-user@18.117.9.159
```

- **If it times out:** The instance may be stopped, the IP may have changed (EC2 public IPs change after stop/start), or your current network may be blocked. Check:
  - AWS Console → EC2 → Instances: instance **running**, note the **Public IPv4 address**.
  - Security group: Inbound rules allow **port 22** from your IP (or `0.0.0.0/0`).
  - Use the **current** public IP in the `ssh` command.

---

## 2. Find where the node is and stop it

Once logged in:

```bash
# Find the pulse-node process
ps aux | grep pulse-node
```

You’ll see a line like:

```text
ec2-user  12345  ...  ./target/release/pulse-node --port 8080
```

Note the **PID** (second column, e.g. `12345`). Then:

```bash
kill 12345
```

(Replace `12345` with the real PID.) Wait a few seconds. If it’s still there:

```bash
kill -9 12345
```

Confirm it’s gone:

```bash
ps aux | grep pulse-node
```

(You should only see the `grep` line.)

---

## 3. Update code (if you deploy from git)

If the Pulse repo is on the instance (e.g. under `/opt/pulse`):

```bash
cd /opt/pulse
git pull
```

If the app lives somewhere else, `cd` to that repo root and `git pull` there.

**Option B: No git on instance – copy code from your Mac**

If you don’t use git on the instance, sync the repo from your Mac, then build on the instance.  
**Important:** The rsync command below must be run **on your Mac** in a **new terminal window** (not inside the SSH session). The key and repo paths are on your Mac; the instance doesn’t have them.

**1. On your Mac** — open a new terminal, leave SSH for a moment if needed. Run:

```bash
cd /Users/amcarbonaro/Desktop/Pulse
rsync -avz --exclude 'target' --exclude 'node_modules' -e "ssh -i ~/Desktop/Pulse/infrastructure/pulse-key.pem" ./ ec2-user@18.117.9.159:~/pulse-node/
```

If you don’t have `rsync`, use `scp` to copy the whole Pulse folder (or at least the `node` directory) to `~/pulse-node/` on the instance.

**2. Back in SSH on the instance:**

```bash
cd /home/ec2-user/pulse-node/node
cargo build --release
```

Then stop the old node (see section 2 for `kill <PID>`), and start the new one (section 5). If the node was running with `--simulate`:

```bash
nohup ./target/release/pulse-node --simulate --port 8080 > pulse.log 2>&1 &
```

---

## 4. Build the node

From the **node** directory (same place that has `Cargo.toml` for the node):

```bash
cd /opt/pulse/node
cargo build --release
```

If you get “no such file or directory”, the node might be elsewhere. Find it:

```bash
find /opt -name "Cargo.toml" 2>/dev/null
```

Then `cd` into the directory that has the node’s `Cargo.toml` and run `cargo build --release` there.

---

## 5. Start the node again

From the same directory where you ran `cargo build --release`:

```bash
nohup ./target/release/pulse-node --port 8080 > pulse.log 2>&1 &
```

Check it’s running:

```bash
ps aux | grep pulse-node
curl -s http://localhost:8080/health
curl -s http://localhost:8080/blocks
```

You should see JSON from `/health` and `/blocks` (e.g. `{"success":true,"data":[...]}` for `/blocks`).

---

## 6. Exit SSH

```bash
exit
```

Then reload the **Chain** page in the frontend; the 404 should be gone if the new binary includes `GET /blocks`.
