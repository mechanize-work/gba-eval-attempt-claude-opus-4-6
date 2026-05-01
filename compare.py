#!/usr/bin/env python3
"""Compare emulator output frames against oracle reference."""
import subprocess
import sys
import os
import numpy as np

def read_ppm(path):
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

def main():
    rom = sys.argv[1] if len(sys.argv) > 1 else "dev-roms/trogdor.gba"
    frames = int(sys.argv[2]) if len(sys.argv) > 2 else 30
    replay = sys.argv[3] if len(sys.argv) > 3 else None

    ref_dir = "/tmp/oracle_ref"
    emu_dir = "/tmp/emu_output"

    os.makedirs(ref_dir, exist_ok=True)
    os.makedirs(emu_dir, exist_ok=True)

    # Run oracle
    print(f"Running oracle on {rom} for {frames} frames...")
    cmd = ["oracle", "run", rom, str(frames), "--dump-frames", ref_dir]
    if replay:
        cmd += ["--replay", replay]
    result = subprocess.run(cmd, capture_output=True, text=True)
    print(f"Oracle: {result.stdout.strip()}")

    # Build and run emulator
    print("Building emulator...")
    result = subprocess.run(
        ["cargo", "build", "--release", "--features", "native-test", "--bin", "test_gba"],
        capture_output=True, text=True
    )
    if result.returncode != 0:
        print(f"Build failed: {result.stderr}")
        return

    print(f"Running emulator on {rom} for {frames} frames...")
    cmd = ["./target/release/test_gba", rom, str(frames), emu_dir]
    if replay:
        cmd.append(replay)
    result = subprocess.run(cmd, capture_output=True, text=True)
    print(result.stderr.strip())

    # Compare
    print(f"\nComparing {frames} frames:")
    total_match = 0
    for i in range(frames):
        ref_path = f"{ref_dir}/frame_{i:05d}.ppm"
        emu_path = f"{emu_dir}/frame_{i:05}.ppm"

        if not os.path.exists(ref_path):
            print(f"  Frame {i}: reference missing")
            continue
        if not os.path.exists(emu_path):
            print(f"  Frame {i}: emulator output missing")
            continue

        ref_rgb = read_ppm(ref_path)
        emu_rgb = read_ppm(emu_path)

        diff = np.abs(ref_rgb.astype(int) - emu_rgb.astype(int))
        if diff.sum() == 0:
            print(f"  Frame {i}: PERFECT")
            total_match += 1
        else:
            pct = (diff > 0).sum() / (240 * 160 * 3) * 100
            print(f"  Frame {i}: {pct:.1f}% differ, max={diff.max()}, avg={diff[diff>0].mean():.1f}")

    print(f"\n{total_match}/{frames} frames match perfectly")

if __name__ == "__main__":
    main()
