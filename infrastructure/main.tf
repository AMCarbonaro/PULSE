# Pulse Network - AWS Infrastructure
# Terraform configuration for genesis bootstrap

terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

provider "aws" {
  region  = "us-east-2"  # Ohio - close to Detroit
  profile = "pulse"
}

# Variables
variable "project_name" {
  default = "pulse-network"
}

variable "environment" {
  default = "dev"
}

variable "alert_email" {
  description = "Email for CloudWatch alarms"
  default     = ""
}

# VPC
resource "aws_vpc" "main" {
  cidr_block           = "10.0.0.0/16"
  enable_dns_hostnames = true
  enable_dns_support   = true

  tags = {
    Name = "${var.project_name}-vpc"
  }
}

# Public Subnet (API endpoints, load balancers)
resource "aws_subnet" "public" {
  vpc_id                  = aws_vpc.main.id
  cidr_block              = "10.0.1.0/24"
  availability_zone       = "us-east-2a"
  map_public_ip_on_launch = true

  tags = {
    Name = "${var.project_name}-public"
  }
}

# Private Subnet (nodes, databases)
resource "aws_subnet" "private" {
  vpc_id            = aws_vpc.main.id
  cidr_block        = "10.0.2.0/24"
  availability_zone = "us-east-2a"

  tags = {
    Name = "${var.project_name}-private"
  }
}

# Internet Gateway
resource "aws_internet_gateway" "main" {
  vpc_id = aws_vpc.main.id

  tags = {
    Name = "${var.project_name}-igw"
  }
}

# Route Table
resource "aws_route_table" "public" {
  vpc_id = aws_vpc.main.id

  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.main.id
  }

  tags = {
    Name = "${var.project_name}-public-rt"
  }
}

resource "aws_route_table_association" "public" {
  subnet_id      = aws_subnet.public.id
  route_table_id = aws_route_table.public.id
}

# Security Group for Node
resource "aws_security_group" "node" {
  name        = "${var.project_name}-node-sg"
  description = "Security group for Pulse nodes"
  vpc_id      = aws_vpc.main.id

  # SSH
  ingress {
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]  # Restrict in production
  }

  # HTTPS API
  ingress {
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # HTTP API
  ingress {
    from_port   = 8080
    to_port     = 8080
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  # P2P libp2p
  ingress {
    from_port   = 4001
    to_port     = 4001
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = {
    Name = "${var.project_name}-node-sg"
  }
}

# ==================== IAM Role for EC2 ====================

# IAM role for EC2 instances
resource "aws_iam_role" "node_role" {
  name = "${var.project_name}-node-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "ec2.amazonaws.com"
        }
      }
    ]
  })

  tags = {
    Name = "${var.project_name}-node-role"
  }
}

# CloudWatch Logs policy
resource "aws_iam_role_policy" "cloudwatch_logs" {
  name = "${var.project_name}-cloudwatch-logs"
  role = aws_iam_role.node_role.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "logs:CreateLogGroup",
          "logs:CreateLogStream",
          "logs:PutLogEvents",
          "logs:DescribeLogStreams"
        ]
        Resource = "arn:aws:logs:*:*:*"
      },
      {
        Effect = "Allow"
        Action = [
          "cloudwatch:PutMetricData"
        ]
        Resource = "*"
      }
    ]
  })
}

# S3 read policy for fetching node binary (for CI/CD later)
resource "aws_iam_role_policy" "s3_artifacts" {
  name = "${var.project_name}-s3-artifacts"
  role = aws_iam_role.node_role.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "s3:GetObject",
          "s3:ListBucket"
        ]
        Resource = [
          aws_s3_bucket.artifacts.arn,
          "${aws_s3_bucket.artifacts.arn}/*"
        ]
      }
    ]
  })
}

# SSM policy for remote management
resource "aws_iam_role_policy_attachment" "ssm_managed" {
  role       = aws_iam_role.node_role.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore"
}

# Instance profile
resource "aws_iam_instance_profile" "node_profile" {
  name = "${var.project_name}-node-profile"
  role = aws_iam_role.node_role.name
}

# ==================== S3 Bucket for Artifacts ====================

resource "aws_s3_bucket" "artifacts" {
  bucket = "${var.project_name}-artifacts-${data.aws_caller_identity.current.account_id}"

  tags = {
    Name = "${var.project_name}-artifacts"
  }
}

resource "aws_s3_bucket_versioning" "artifacts" {
  bucket = aws_s3_bucket.artifacts.id
  versioning_configuration {
    status = "Enabled"
  }
}

resource "aws_s3_bucket_server_side_encryption_configuration" "artifacts" {
  bucket = aws_s3_bucket.artifacts.id
  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

data "aws_caller_identity" "current" {}

# ==================== CloudWatch ====================

# Log group for node logs
resource "aws_cloudwatch_log_group" "node_logs" {
  name              = "/pulse-network/node"
  retention_in_days = 30

  tags = {
    Name = "${var.project_name}-node-logs"
  }
}

# SNS topic for alarms (if email provided)
resource "aws_sns_topic" "alerts" {
  count = var.alert_email != "" ? 1 : 0
  name  = "${var.project_name}-alerts"
}

resource "aws_sns_topic_subscription" "email" {
  count     = var.alert_email != "" ? 1 : 0
  topic_arn = aws_sns_topic.alerts[0].arn
  protocol  = "email"
  endpoint  = var.alert_email
}

# ==================== CloudWatch Alarms ====================

# Alarm: EC2 instance status check failed
resource "aws_cloudwatch_metric_alarm" "node_status" {
  alarm_name          = "${var.project_name}-node-status"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 2
  metric_name         = "StatusCheckFailed"
  namespace           = "AWS/EC2"
  period              = 60
  statistic           = "Maximum"
  threshold           = 0
  alarm_description   = "Pulse node EC2 instance status check failed"
  
  dimensions = {
    InstanceId = aws_instance.genesis_node.id
  }

  alarm_actions = var.alert_email != "" ? [aws_sns_topic.alerts[0].arn] : []
  ok_actions    = var.alert_email != "" ? [aws_sns_topic.alerts[0].arn] : []
}

# ==================== EC2 Instance ====================

resource "aws_instance" "genesis_node" {
  ami                  = "ami-05efc83cb5512477c"  # Amazon Linux 2023 (us-east-2)
  instance_type        = "t3.micro"                # Free tier eligible
  subnet_id            = aws_subnet.public.id
  key_name             = "pulse-key"
  iam_instance_profile = aws_iam_instance_profile.node_profile.name

  vpc_security_group_ids = [aws_security_group.node.id]

  root_block_device {
    volume_size = 30
    volume_type = "gp3"
  }

  tags = {
    Name = "${var.project_name}-genesis-node"
  }

  user_data = <<-EOF
              #!/bin/bash
              set -e
              
              # Update and install deps
              dnf update -y
              dnf install -y gcc git
              
              # Install Rust
              curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
              source ~/.cargo/env
              
              # Install CloudWatch agent
              dnf install -y amazon-cloudwatch-agent
              
              # Create pulse directories
              mkdir -p /opt/pulse/data
              mkdir -p /opt/pulse/logs
              
              # Signal ready
              echo "Bootstrap complete" > /opt/pulse/ready
              EOF

  lifecycle {
    ignore_changes = [user_data]  # Don't replace instance on user_data changes
  }
}

# ==================== Outputs ====================

output "genesis_node_public_ip" {
  value = aws_instance.genesis_node.public_ip
}

output "vpc_id" {
  value = aws_vpc.main.id
}

output "artifacts_bucket" {
  value = aws_s3_bucket.artifacts.bucket
}

output "log_group" {
  value = aws_cloudwatch_log_group.node_logs.name
}
