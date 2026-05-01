#!/bin/bash
# Test the emulator against oracle by building a native test binary
# Usage: ./test_emu.sh <rom> <frames>

ROM=${1:-dev-roms/trogdor.gba}
FRAMES=${2:-30}
REF_DIR=/tmp/oracle_ref
EMU_DIR=/tmp/emu_output

mkdir -p "$REF_DIR" "$EMU_DIR"

echo "=== Running oracle on $ROM for $FRAMES frames ==="
oracle run "$ROM" "$FRAMES" --dump-frames "$REF_DIR"

echo "=== Building and running emulator ==="
# We need to build a native test binary that links against gba_emu as rlib
cargo build --release --lib 2>&1 | tail -5

# Create test binary
cat > /tmp/test_gba.rs << 'RUSTEOF'
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: test_gba <rom> <frames> <output_dir>");
        std::process::exit(1);
    }
    let rom_path = &args[1];
    let frames: usize = args[2].parse().unwrap();
    let output_dir = &args[3];

    // We would need to link against gba_emu, but that's built for wasm
    // This approach won't work directly
    eprintln!("Cannot run wasm binary natively. Need wasmtime.");
}
RUSTEOF

echo "Need alternative approach..."
