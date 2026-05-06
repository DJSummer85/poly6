#!/bin/bash

set -e

PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
BACKEND_DIR="$PROJECT_DIR/backend"
FRONTEND_DIR="$PROJECT_DIR/frontend"

echo "Starting polymarket-trader dev environment..."
echo "============================================"

# Check for .env file
if [ ! -f "$PROJECT_DIR/.env" ]; then
    echo "Warning: .env file not found. Creating from .env.example..."
    if [ -f "$PROJECT_DIR/.env.example" ]; then
        cp "$PROJECT_DIR/.env.example" "$PROJECT_DIR/.env"
        echo "Created .env - Please edit and set JWT_SECRET!"
    else
        echo "Error: .env.example not found. Please create .env manually."
        exit 1
    fi
fi

cleanup() {
    echo ""
    echo "Shutting down..."
    kill $BACKEND_PID $FRONTEND_PID 2>/dev/null || true
    exit 0
}

trap cleanup SIGINT SIGTERM

# Ensure database directory exists
mkdir -p "$BACKEND_DIR/data"

echo "Starting backend (port 3001)..."
cd "$BACKEND_DIR"
cargo run --release &
BACKEND_PID=$!

sleep 3

echo "Starting frontend (port 3000)..."
cd "$FRONTEND_DIR"
bun run dev &
FRONTEND_PID=$!

echo ""
echo "============================================"
echo "Backend:  http://localhost:3001"
echo "Frontend: http://localhost:3000"
echo "============================================"
echo "Press Ctrl+C to stop both services"
echo ""

wait