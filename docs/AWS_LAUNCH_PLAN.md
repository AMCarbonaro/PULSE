# AWS Production Launch Plan

## Prerequisites

- AWS account with EC2, VPC, RDS, S3, IAM, Lambda, CloudFront, CloudWatch, Route 53
- Domain name for node endpoints
- Mobile app environment (iOS/Android)
- Code repository ready

---

## Step 1: AWS Infrastructure Setup

### 1.1 Networking (VPC & Subnets)
- Create VPC for nodes
- 2–3 subnets:
  - **Public**: API endpoints, load balancers
  - **Private**: Nodes, databases, ledger storage
- Security groups for TCP/UDP: P2P, gRPC, WebSockets

### 1.2 Node Cluster
- EC2 instances: `m5.large` or `c5.large` to start
- Auto-scaling group for production
- Install node binaries (Rust/Go)
- Configure peer discovery and P2P

### 1.3 Ledger Database
- RDS/PostgreSQL or DynamoDB
- S3 for block archive backups
- Multi-AZ replication

### 1.4 Load Balancing & DNS
- Application Load Balancer (ALB) for APIs
- Route53 for domain management

### 1.5 Monitoring
- CloudWatch for logs, metrics, alarms
- Optional: Prometheus/Grafana

---

## Step 2: First Node Deployment

- Deploy seed node with local ledger
- Accept device heartbeat packets
- PoL threshold = 1 (solo genesis)
- Verify end-to-end signing/verification

---

## Step 3: Device / SDK Setup

- Install SDK on phone/watch
- Configure device private key
- Connect to node API
- Start streaming signed heartbeats

---

## Step 4: First Pulse Block

- Node aggregates heartbeat(s)
- Weighted contribution (100% for you)
- Commit genesis block
- Verify ledger state

---

## Step 5: Token Initialization

- Generate first Pulse Tokens
- Assign to verified heartbeat
- Test self-transaction

---

## Step 6: Multi-Node Expansion

- Add EC2 nodes in other regions
- Sync ledger across nodes
- Test block propagation

---

## Step 7: Monitoring & Analytics

- Real-time dashboards:
  - Active heartbeat count
  - TPS
  - Block confirmation times
- Audit logging

---

## Step 8: Scaling

- Multi-device redundancy
- Offline/reconnect scenarios
- Tune thresholds and weights

---

## AWS Services Summary

| Service | Purpose |
|---------|---------|
| EC2 | Node compute |
| VPC | Network isolation |
| RDS/DynamoDB | Ledger storage |
| S3 | Block archives |
| ALB | API load balancing |
| Route53 | DNS |
| CloudWatch | Monitoring |
| Lambda | Automation |
| IAM | Access control |

---

## Cost Estimates

| Phase | Monthly Cost |
|-------|--------------|
| MVP (1 user) | $50–200 |
| Early (1k users) | $200–500 |
| Growth (1M users) | $1k–10k |
