# What a Full AWS Deployment Would Look Like

Right now Pulse has **partial** AWS: Terraform spins up a VPC, subnets, one EC2 instance (genesis node), and security groups. You still **SSH in, build the Rust binary by hand, and run the node**. There is no ALB, no automatic HTTPS with your own domain, no multi-node cluster, no automated deploy pipeline, and no formal monitoring. This doc describes what "full" would be and what’s left to build.

---

## Current State (What Exists)

| Component | Status |
|-----------|--------|
| VPC + public/private subnets (us-east-2) | Done in Terraform |
| Security group (SSH 22, HTTPS 443, API 8080, P2P 4001) | Done |
| Single EC2 instance (t3.micro, Amazon Linux 2023) | Done |
| User data | Installs Rust + creates /opt/pulse; does **not** build or run pulse-node |
| Node binary on instance | Manual: you rsync/Clone repo, build, run (see RESTART-NODE.md) |
| HTTPS for node | Manual: Cloudflare Tunnel (see HTTPS-NODE-CLOUDFLARE.md) or nothing |
| Domain / DNS | None in Terraform |
| Load balancer | None |
| Multi-node / P2P sync | Not in infra; node code has P2P hooks but multi-node sync not production-ready |
| Ledger persistence | Node uses sled in `--data-dir` (local disk). No RDS, no S3 backup in Terraform |
| Monitoring / logging | None in Terraform |
| CI/CD | None: no automated build/deploy of node or app |

So today: **one EC2 node, manually built and run, optional Cloudflare Tunnel for HTTPS.**

---

## Full AWS Deployment: Target Picture

A "full" deployment would look like the following. You can implement it in stages.

### 1. Single-node production (next step)

- **Automated node install**
  - Terraform user_data (or a custom AMI / startup script) that:
    - Builds `pulse-node` from a known tag or artifact, or
    - Downloads a pre-built binary from S3 (built by CI), and
    - Runs the node with `--data-dir /opt/pulse/data` (or similar), optionally `--simulate` off.
  - Optionally: systemd unit so the node restarts on reboot and is managed cleanly.
- **EBS for ledger**
  - Node already uses `--data-dir`; put it on the root volume or a separate EBS volume so chain data survives restarts. No RDS required for a single node (sled is enough).
- **HTTPS with your domain**
  - **Option A:** Application Load Balancer (ALB) in front of the EC2 instance, TLS termination at ALB (cert from ACM), Route53 A/AAAA or CNAME pointing your domain (e.g. `api.pulse.example.com`) to the ALB. Node listens on 8080; ALB forwards 443 → 8080.
  - **Option B:** Keep Cloudflare Tunnel on the instance for HTTPS without managing ALB/ACM (simpler, less "AWS-native").
- **Secrets / config**
  - Any API keys or config (if needed later) in AWS Secrets Manager or SSM Parameter Store; instance role to read them (no keys in user_data).
- **Restrict SSH**
  - Tighten security group: SSH only from your IP or a bastion, not `0.0.0.0/0`.

Deliverable: **One node, provisioned by Terraform, that builds/runs the binary and is reachable over HTTPS at a stable domain.**

---

### 2. Multi-node + high availability (later)

- **More EC2 nodes**
  - Second (and Nth) instance(s) in same or other AZs/regions. Same security group rules for 8080 and 4001 (P2P).
  - Node’s P2P (libp2p) must be fully wired and tested so nodes discover each other and sync blocks. Today that’s not fully production-ready; infra can prepare N instances, but sync logic is in the node code.
- **Load balancing**
  - ALB in front of all node instances (target group by instance or private IP). Clients hit one URL; ALB spreads load. Health checks on `/health`.
- **Shared state (optional)**
  - For a single logical chain, each node can keep using sled + EBS. For a shared DB design you’d add RDS/PostgreSQL or DynamoDB and change the node to read/write there; that’s a larger change and not required for the first multi-node version.
- **Discovery**
  - Seed list or Route53 private DNS so nodes find each other for P2P (e.g. `pulse-node-1.internal`, `pulse-node-2.internal`).

Deliverable: **Several nodes behind an ALB, P2P sync working, one public HTTPS endpoint.**

---

### 3. Persistence, backup, and ops

- **EBS**
  - Attach a dedicated EBS volume for `--data-dir` if you want to snapshot/restore without touching root. Enable encryption if required.
- **Backups**
  - Periodic snapshots of the EBS volume(s) holding chain data, or a script that exports blocks to **S3** (e.g. JSON/parquet) for archive. Terraform: S3 bucket + IAM role for instance; cron or Lambda to copy/snapshot.
- **Logs**
  - Ship node stdout/stderr to **CloudWatch Logs** (e.g. cloudwatch-agent on the instance, or run node under a logger that writes to a file and ship that file). Structured logs (e.g. JSON) help.
- **Metrics and alarms**
  - **CloudWatch:** custom metrics (e.g. heartbeats/min, blocks/min, TPS) if the node exposes them (or a small sidecar that scrapes `/stats` and pushes). Alarms on "node down" (health check failing) or "no blocks in N minutes".
  - Optional: **Prometheus + Grafana** on EC2 or in ECS, scraping node metrics if you add a `/metrics` endpoint.

Deliverable: **Durable ledger, backups to S3, logs in CloudWatch, basic alarms.**

---

### 4. CI/CD and app hosting

- **Node**
  - **CI (e.g. GitHub Actions):** on push/tag, build `pulse-node` for Linux (e.g. x86_64-unknown-linux-gnu), run tests, upload binary to **S3** (e.g. `s3://pulse-artifacts/nodes/linux/pulse-node-<version>`).
  - **Deploy:** Terraform or a small script: pull binary from S3 on instance boot or on demand, restart node. Or bake binary into an AMI and roll new instances.
- **Web app (your "little frontend")**
  - Already deployable to **Vercel/Netlify** (static build). For "full AWS" you could instead serve the same static build from **S3 + CloudFront** and optionally Route53 (e.g. `app.pulse.example.com`). No backend required; app talks to node API (your ALB URL).

Deliverable: **Tag → build → artifact in S3; instances (or AMI) run that build. App on S3/CloudFront or keep Vercel/Netlify.**

---

### 5. Summary: What’s done vs not

| Piece | Done? | Full deployment would add |
|-------|--------|----------------------------|
| VPC, subnets, security group | Yes (Terraform) | Restrict SSH; optional second AZ |
| One EC2 node | Yes | User_data or AMI that builds/runs node; systemd; EBS for data |
| HTTPS | Manual (Cloudflare) | ALB + ACM + Route53 for your domain |
| Domain / DNS | No | Route53 (and optionally hosted zone) |
| Multi-node | No | More EC2s, ALB target group, P2P working in node |
| Ledger persistence | Node has sled | EBS volume for data-dir; optional S3 backup |
| Monitoring | No | CloudWatch Logs + metrics + alarms |
| CI/CD | No | Build node in CI → S3; deploy from S3 or AMI |
| App hosting | Vercel/Netlify | Optional: S3 + CloudFront on AWS |

---

## Suggested order of work

1. **Automate node on the existing instance** – user_data or script that builds or fetches `pulse-node` and runs it (systemd), with `--data-dir` on a path that survives reboots.
2. **ALB + ACM + Route53** – one domain for the node API, HTTPS, so you can retire Cloudflare Tunnel if you want.
3. **CloudWatch logs + health alarm** – so you know when the node is down.
4. **CI:** build node → S3; optional deploy step that updates the instance.
5. Later: second node, ALB target group, P2P sync in node code, then backups and more metrics.

That’s what a full AWS deployment would look like; the launch plan doc (docs/AWS_LAUNCH_PLAN.md) lines up with this. We haven’t completed it yet in code or Terraform beyond the base VPC and single EC2.
