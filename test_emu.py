#!/usr/bin/env python3
"""Test emulator against oracle output."""
import subprocess
import sys
import struct
import os
import numpy as np

WASM_PATH = "target/wasm32-unknown-unknown/release/gba_emu.wasm"

def run_oracle(rom, frames, replay=None, dump_dir=None, dump_audio=None):
    cmd = ["oracle", "run", rom, str(frames)]
    if replay:
        cmd += ["--replay", replay]
    if dump_dir:
        os.makedirs(dump_dir, exist_ok=True)
        cmd += ["--dump-frames", dump_dir]
    if dump_audio:
        cmd += ["--dump-audio", dump_audio]
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        print(f"Oracle error: {result.stderr}")
        return None
    return result.stdout

def read_ppm(path):
    """Read a PPM file and return raw RGB data."""
    with open(path, 'rb') as f:
        magic = f.readline().strip()
        while True:
            line = f.readline().strip()
            if not line.startswith(b'#'):
                break
        w, h = map(int, line.split())
        maxval = int(f.readline().strip())
        data = f.read()
    return np.frombuffer(data, dtype=np.uint8).reshape(h, w, 3)

def compare_frames(ref_dir, emu_dir, frame_count):
    """Compare reference and emulator frames."""
    total_diff = 0
    total_pixels = 0
    max_diff = 0

    for i in range(frame_count):
        ref_path = os.path.join(ref_dir, f"frame_{i:05d}.ppm")
        emu_path = os.path.join(emu_dir, f"frame_{i:05d}.ppm")

        if not os.path.exists(ref_path):
            continue
        if not os.path.exists(emu_path):
            print(f"  Frame {i}: MISSING from emulator")
            continue

        ref_img = read_ppm(ref_path)
        emu_img = read_ppm(emu_path)

        diff = np.abs(ref_img.astype(int) - emu_img.astype(int))
        frame_diff = diff.sum()
        frame_max = diff.max()
        pixel_count = 240 * 160

        if frame_diff > 0:
            pct = (diff > 0).sum() / (pixel_count * 3) * 100
            print(f"  Frame {i}: {pct:.1f}% pixels differ, max diff={frame_max}, avg diff={diff[diff>0].mean():.1f}")
        else:
            print(f"  Frame {i}: PERFECT MATCH")

        total_diff += frame_diff
        total_pixels += pixel_count * 3
        max_diff = max(max_diff, frame_max)

    if total_pixels > 0:
        avg = total_diff / total_pixels
        print(f"\n  Overall: avg diff={avg:.3f}, max diff={max_diff}")

if __name__ == "__main__":
    rom = sys.argv[1] if len(sys.argv) > 1 else "dev-roms/trogdor.gba"
    frames = int(sys.argv[2]) if len(sys.argv) > 2 else 10

    print(f"Testing {rom} for {frames} frames")

    ref_dir = "/tmp/test_ref"
    print("Running oracle...")
    run_oracle(rom, frames, dump_dir=ref_dir)

    print("Done. Comparing would require running emulator via wasmtime.")
    print("Reference frames saved to", ref_dir)
