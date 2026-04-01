#!/bin/bash
# Launch 4 clients for multi-team testing via the lobby.
# Each window is positioned in a screen corner (top-left, top-right, bottom-left, bottom-right).
#
#   ./run_4player.sh              # 4 clients → production lobby
#   ./run_4player.sh local        # 4 clients → local Firebase emulator

trap 'kill 0; exit' SIGINT SIGTERM

if [ "$1" = "local" ]; then
    LOBBY="--lobby-api http://127.0.0.1:5001"
    echo "Using local Firebase emulator"
else
    LOBBY=""
    echo "Using production lobby"
fi

# Get screen dimensions via AppleScript
SCREEN_INFO=$(osascript -e 'tell application "Finder" to get bounds of window of desktop' 2>/dev/null)
if [ -n "$SCREEN_INFO" ]; then
    SCREEN_W=$(echo "$SCREEN_INFO" | awk -F', ' '{print $3}')
    SCREEN_H=$(echo "$SCREEN_INFO" | awk -F', ' '{print $4}')
else
    SCREEN_W=1512
    SCREEN_H=982
fi

HALF_W=$((SCREEN_W / 2))
HALF_H=$((SCREEN_H / 2))
MENU_BAR=25

echo "Building client..."
cargo build --bin client 2>&1 || exit 1

echo "Screen: ${SCREEN_W}x${SCREEN_H}, each window: ${HALF_W}x$((HALF_H - MENU_BAR))"

# Corner positions: x y width height
POSITIONS_X=(0 $HALF_W 0 $HALF_W)
POSITIONS_Y=($MENU_BAR $MENU_BAR $HALF_H $HALF_H)
WIDTHS=($HALF_W $HALF_W $HALF_W $HALF_W)
HEIGHTS=($((HALF_H - MENU_BAR)) $((HALF_H - MENU_BAR)) $((SCREEN_H - HALF_H)) $((SCREEN_H - HALF_H)))

CLIENT_PIDS=()

for i in 0 1 2 3; do
    echo "=== Starting client $((i + 1)) (Player$((i + 1))) ==="
    cargo run --bin client -- --name "Player$((i + 1))" $LOBBY &
    CLIENT_PIDS+=($!)
    sleep 2
done

# Wait for all windows to be ready, then position them by PID
echo "Waiting for windows to appear..."
sleep 4

for i in 0 1 2 3; do
    pid=${CLIENT_PIDS[$i]}
    x=${POSITIONS_X[$i]}
    y=${POSITIONS_Y[$i]}
    w=${WIDTHS[$i]}
    h=${HEIGHTS[$i]}

    osascript -e "
        tell application \"System Events\"
            set clientProcs to every process whose unix id is ${pid}
            if (count of clientProcs) > 0 then
                set targetProc to item 1 of clientProcs
                if (count of windows of targetProc) > 0 then
                    set targetWin to window 1 of targetProc
                    set position of targetWin to {${x}, ${y}}
                    set size of targetWin to {${w}, ${h}}
                end if
            end if
        end tell
    " 2>/dev/null && echo "Positioned window $((i + 1))" || echo "Warning: could not position window $((i + 1))"
done

echo ""
echo "All 4 clients launched. Create/join games via the lobby UI."
echo "Press Ctrl+C to kill all clients."
wait
