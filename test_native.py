#!/usr/bin/env python3
"""Test GBA emulator using oracle session API to compare frame-by-frame."""
import subprocess
import json
import sys
import os
import struct
import numpy as np

def oracle_session_test(rom, frames, replay=None):
    """Run oracle via session API and dump frames."""
    ref_dir = "/tmp/oracle_ref"
    os.makedirs(ref_dir, exist_ok=True)

    cmd = ["oracle", "run", rom, str(frames), "--dump-frames", ref_dir]
    if replay:
        cmd += ["--replay", replay]
    result = subprocess.run(cmd, capture_output=True, text=True)
    print(f"Oracle: {result.stdout.strip()}")
    return ref_dir

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

def write_ppm(path, rgb_array):
    h, w, _ = rgb_array.shape
    with open(path, 'wb') as f:
        f.write(f"P6\n{w} {h}\n255\n".encode())
        f.write(rgb_array.tobytes())

if __name__ == "__main__":
    rom = sys.argv[1] if len(sys.argv) > 1 else "dev-roms/trogdor.gba"
    frames = int(sys.argv[2]) if len(sys.argv) > 2 else 5

    ref_dir = oracle_session_test(rom, frames)
    for i in range(min(frames, 3)):
        ref_path = f"{ref_dir}/frame_{i:05d}.ppm"
        if os.path.exists(ref_path):
            img = read_ppm(ref_path)
            # Check if it's not all black/white
            print(f"Frame {i}: min={img.min()}, max={img.max()}, mean={img.mean():.1f}")
            # Show top-left corner colors
            print(f"  Top-left pixel: R={img[0,0,0]} G={img[0,0,1]} B={img[0,0,2]}")
            print(f"  Center pixel: R={img[80,120,0]} G={img[80,120,1]} B={img[80,120,2]}")
