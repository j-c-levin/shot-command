#!/bin/bash
# Launch the map editor. Optional: pass a map filename to edit.
# Usage: ./run_editor.sh [map_name.ron]

cargo build --bin client 2>&1 || exit 1

if [ -n "$1" ]; then
    echo "=== Opening editor with map: $1 ==="
    cargo run --bin client -- --editor --map "$1"
else
    echo "=== Opening editor (new map) ==="
    cargo run --bin client -- --editor
fi
