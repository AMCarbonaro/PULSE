# Expose Pulse Node over HTTPS (Cloudflare Tunnel)

So the Netlify app (HTTPS) can connect to your node, the node must be reachable over **HTTPS**. A quick way with no domain or certificates is **Cloudflare Quick Tunnel**.

---

## 1. SSH into your EC2 instance

```bash
ssh -i ~/Desktop/Pulse/infrastructure/pulse-key.pem ec2-user@18.117.9.159
```

(Use your instance’s current public IP if different.)

---

## 2. Install cloudflared on the instance

**Amazon Linux 2023:**

```bash
sudo rpm -ivh https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-x86_64.rpm
```

If that fails (e.g. 404), get the latest URL from:  
https://github.com/cloudflare/cloudflared/releases  
and use the `cloudflared-linux-x86_64.rpm` link.

**Alternative (binary in your home dir):**

```bash
wget -q https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-x86_64 -O cloudflared
chmod +x cloudflared
./cloudflared --version
```

---

## 3. Start the tunnel (node must be running)

Ensure the Pulse node is running on the instance (e.g. on port 8080). Then:

```bash
cloudflared tunnel --url http://localhost:8080
```

Or if you installed the binary in your home dir:

```bash
./cloudflared tunnel --url http://localhost:8080
```

Leave this running. The first time it will print something like:

```text
Your quick Tunnel has been created! Visit it at:
https://random-words-here.trycloudflare.com
```

Copy that **https://** URL.

---

## 4. Use the HTTPS URL in the app

1. Open your app on Netlify (HTTPS).
2. In **Node URL**, paste the tunnel URL (e.g. `https://random-words-here.trycloudflare.com`). Do **not** add a path or port.
3. Click **Connect to node**.

The app will talk to your node over HTTPS and the browser will allow it.

---

## 5. Keep the tunnel running (optional)

- **Quick test:** Leave the terminal with `cloudflared tunnel --url ...` open. If you close SSH or stop the process, the URL stops working.
- **Longer term:** Run cloudflared in the background or as a service, e.g.:

  ```bash
  nohup cloudflared tunnel --url http://localhost:8080 > cloudflared.log 2>&1 &
  ```

  The **URL changes every time** you start a new quick tunnel. So after a reboot you’ll get a new URL and need to paste it again in the app.

For a **fixed URL** and restarts without changing the link, use a [named Cloudflare Tunnel](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/do-more-with-tunnels/) (needs a domain and Cloudflare account).

---

## Summary

| Step | Action |
|------|--------|
| 1 | SSH to EC2 |
| 2 | Install `cloudflared` |
| 3 | Run `cloudflared tunnel --url http://localhost:8080` (node must be up on 8080) |
| 4 | Copy the `https://....trycloudflare.com` URL |
| 5 | In the Netlify app, set Node URL to that URL and click Connect |

After that, the app on Netlify can use the node over HTTPS.
