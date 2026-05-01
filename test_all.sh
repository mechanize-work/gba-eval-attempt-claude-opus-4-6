#!/bin/bash
# Test all 5 games against oracle, report per-game and total scores
set -e

GAMES="anguna another-world meteorain trogdor xniq"
FRAMES=30
TOTAL_MATCH=0
TOTAL_FRAMES=0

cargo build --release --features native-test --target x86_64-unknown-linux-gnu 2>/dev/null

for game in $GAMES; do
    ROM="dev-roms/${game}.gba"
    EMU_DIR="/tmp/emu_${game}"
    ORACLE_DIR="/tmp/oracle_${game}"

    rm -rf "$EMU_DIR" "$ORACLE_DIR"
    mkdir -p "$EMU_DIR" "$ORACLE_DIR"

    # Run emulator
    ./target/x86_64-unknown-linux-gnu/release/test_gba "$ROM" $FRAMES "$EMU_DIR" 2>/dev/null

    # Run oracle
    oracle run "$ROM" $FRAMES --dump-frames "$ORACLE_DIR" >/dev/null 2>&1

    # Compare frames
    MATCH=0
    for i in $(seq 0 $((FRAMES-1))); do
        EMU_F=$(printf "%s/frame_%05d.ppm" "$EMU_DIR" "$i")
        ORACLE_F=$(printf "%s/frame_%05d.ppm" "$ORACLE_DIR" "$i")
        if [ -f "$EMU_F" ] && [ -f "$ORACLE_F" ]; then
            if cmp -s "$EMU_F" "$ORACLE_F"; then
                MATCH=$((MATCH+1))
            fi
        fi
    done

    echo "$game: $MATCH/$FRAMES"
    TOTAL_MATCH=$((TOTAL_MATCH+MATCH))
    TOTAL_FRAMES=$((TOTAL_FRAMES+FRAMES))
done

echo ""
echo "TOTAL: $TOTAL_MATCH/$TOTAL_FRAMES"
