#!/bin/bash
# Spin up server + 2 clients, show all logs interleaved.
# Ctrl+C kills everything.

trap 'kill 0; exit' SIGINT SIGTERM

cargo build --bin server --bin client 2>&1 || exit 1

echo "=== Starting server ==="
cargo run --bin server 2>&1 | sed 's/^/[SERVER] /' &
sleep 2

echo "=== Starting client 1 ==="
cargo run --bin client 2>&1 | sed 's/^/[CLIENT1] /' &
sleep 3

echo "=== Starting client 2 ==="
cargo run --bin client 2>&1 | sed 's/^/[CLIENT2] /' &

wait
