#!/usr/bin/env python3
"""Test GBA emulator wasm against oracle reference."""
import subprocess
import sys
import os
import struct
import json
import numpy as np

WASM = "target/wasm32-unknown-unknown/release/gba_emu.wasm"

def run_wasm_emulator(rom_path, frames, replay=None):
    """Run the emulator via wasmtime and capture framebuffer output."""
    # We need a small wrapper to call the wasm exports
    # Let's use wasmtime's Python bindings or a simple C harness
    # For now, let's write a quick test using oracle session for comparison
    pass

def run_oracle_session(rom_path, frames, replay_data=None):
    """Run oracle and capture each frame's framebuffer."""
    result = subprocess.run(
        ["oracle", "session", "start", rom_path],
        capture_output=True, text=True
    )
    session = json.loads(result.stdout)
    sid = session["id"]

    # Parse replay data
    key_events = {}
    if replay_data:
        for line in replay_data.strip().split('\n'):
            parts = line.strip().split()
            if len(parts) >= 2:
                frame_num = int(parts[0])
                keys = int(parts[1], 16)
                key_events[frame_num] = keys

    framebuffers = []
    for f in range(frames):
        if f in key_events:
            subprocess.run(
                ["oracle", "session", "set-keys", sid, hex(key_events[f])],
                capture_output=True
            )
        subprocess.run(
            ["oracle", "session", "run-frame", sid, "1"],
            capture_output=True
        )
        result = subprocess.run(
            ["oracle", "session", "framebuffer", sid],
            capture_output=True
        )
        framebuffers.append(result.stdout)

    subprocess.run(["oracle", "session", "end", sid], capture_output=True)
    return framebuffers

def oracle_dump(rom_path, frames, replay=None):
    """Use oracle run with dump-frames."""
    ref_dir = "/tmp/oracle_ref"
    os.makedirs(ref_dir, exist_ok=True)
    cmd = ["oracle", "run", rom_path, str(frames), "--dump-frames", ref_dir]
    if replay:
        cmd += ["--replay", replay]
    result = subprocess.run(cmd, capture_output=True, text=True)
    print(f"Oracle: {result.stdout.strip()}")
    return ref_dir

if __name__ == "__main__":
    rom = sys.argv[1] if len(sys.argv) > 1 else "dev-roms/trogdor.gba"
    frames = int(sys.argv[2]) if len(sys.argv) > 2 else 5

    print(f"Running oracle on {rom} for {frames} frames...")
    ref_dir = oracle_dump(rom, frames)

    # Check reference frames
    for i in range(frames):
        path = f"{ref_dir}/frame_{i:05d}.ppm"
        if os.path.exists(path):
            size = os.path.getsize(path)
            print(f"  Frame {i}: {size} bytes")
