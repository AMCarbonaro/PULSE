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

  # gRPC / P2P
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

# EC2 Instance for Genesis Node (Free Tier: t3.micro = 750 hrs/mo for 12 mo)
resource "aws_instance" "genesis_node" {
  ami           = "ami-05efc83cb5512477c"  # Amazon Linux 2023 (us-east-2)
  instance_type = "t3.micro"                # Free tier eligible
  subnet_id     = aws_subnet.public.id
  key_name      = "pulse-key"

  vpc_security_group_ids = [aws_security_group.node.id]

  root_block_device {
    volume_size = 30  # Keep existing size (can't shrink in-place); 30 GB = free tier limit
    volume_type = "gp3"
  }

  tags = {
    Name = "${var.project_name}-genesis-node"
  }

  # User data to bootstrap node
  user_data = <<-EOF
              #!/bin/bash
              set -e
              
              # Update and install deps
              dnf update -y
              dnf install -y gcc git
              
              # Install Rust
              curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
              source ~/.cargo/env
              
              # Create pulse user
              useradd -m pulse
              mkdir -p /opt/pulse
              chown pulse:pulse /opt/pulse
              
              # Signal ready
              echo "Bootstrap complete" > /opt/pulse/ready
              EOF
}

# Outputs
output "genesis_node_public_ip" {
  value = aws_instance.genesis_node.public_ip
}

output "vpc_id" {
  value = aws_vpc.main.id
}
