#!/usr/bin/env bash
set -euo pipefail

echo "=== Starting CCTraveler Dev Services ==="

# Start Python scraper in background
echo ">> Starting scraper service on :8300..."
cd services/scraper
if [ -f .venv/bin/activate ]; then
    source .venv/bin/activate
elif [ -f venv/bin/activate ]; then
    source venv/bin/activate
else
    echo "!! no virtualenv found in services/scraper (.venv or venv) — run 'pnpm setup:python' first" >&2
    exit 1
fi
uvicorn src.server:app --port 8300 --reload &
SCRAPER_PID=$!
cd ../..

# Start Next.js frontend in background
echo ">> Starting web frontend on :3000..."
cd packages/web
pnpm dev &
WEB_PID=$!
cd ../..

echo ""
echo "Services running:"
echo "  Scraper:  http://localhost:8300"
echo "  Frontend: http://localhost:3100"
echo ""
echo "Press Ctrl+C to stop all services."

trap "kill $SCRAPER_PID $WEB_PID 2>/dev/null; exit" INT TERM
wait
