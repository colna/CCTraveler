#!/usr/bin/env bash
set -euo pipefail

echo "=== CCTraveler Setup ==="

echo ">> Installing Node dependencies..."
pnpm install

echo ">> Building Rust workspace..."
cargo build --workspace

echo ">> Setting up Python scraper..."
cd services/scraper
python3 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
cd ../..

echo ">> Creating data directory..."
mkdir -p data

echo "=== Setup complete ==="
echo "Run 'pnpm dev' or 'scripts/dev.sh' to start all services."
