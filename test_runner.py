#!/usr/bin/env python3
"""Test GBA emulator wasm against oracle, using ctypes to call wasmtime C API."""
import subprocess
import sys
import os
import struct
import json
import numpy as np
import ctypes
import ctypes.util

def find_wasmtime_lib():
    """Find wasmtime shared library."""
    for path in [
        "/usr/local/lib/libwasmtime.so",
        "/usr/lib/libwasmtime.so",
    ]:
        if os.path.exists(path):
            return path
    result = subprocess.run(["find", "/", "-name", "libwasmtime*", "-type", "f"],
                          capture_output=True, text=True, timeout=5)
    for line in result.stdout.strip().split('\n'):
        if line.endswith('.so') or line.endswith('.dylib'):
            return line
    return None

# Instead of using wasmtime C API (complex), let's compile a native Rust test binary
def build_and_test():
    """Build a native test binary and run it."""
    pass

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

def fb_to_rgb(fb_bytes):
    """Convert AABBGGRR (0xAABBGGRR) framebuffer bytes to RGB numpy array."""
    pixels = np.frombuffer(fb_bytes, dtype=np.uint32).reshape(160, 240)
    r = (pixels & 0xFF).astype(np.uint8)
    g = ((pixels >> 8) & 0xFF).astype(np.uint8)
    b = ((pixels >> 16) & 0xFF).astype(np.uint8)
    return np.stack([r, g, b], axis=2)

def write_ppm(path, rgb_array):
    h, w, _ = rgb_array.shape
    with open(path, 'wb') as f:
        f.write(f"P6\n{w} {h}\n255\n".encode())
        f.write(rgb_array.tobytes())

if __name__ == "__main__":
    lib = find_wasmtime_lib()
    print(f"Wasmtime lib: {lib}")
