#!/usr/bin/env python3
"""Compare GBA emulator wasm output against oracle reference frames."""
import subprocess
import sys
import os
import struct
import json
import numpy as np
from wasmtime import Store, Module, Instance, Memory, Func, FuncType, ValType

WASM = "target/wasm32-unknown-unknown/release/gba_emu.wasm"

def load_emulator(wasm_path):
    store = Store()
    module = Module.from_file(store.engine, wasm_path)
    instance = Instance(store, module, [])
    return store, instance

def run_emulator(rom_path, frames, replay=None):
    store, instance = load_emulator(WASM)

    exports = instance.exports(store)
    emu_init = exports["emu_init"]
    emu_rom_buffer = exports["emu_rom_buffer"]
    emu_load_rom = exports["emu_load_rom"]
    emu_set_keys = exports["emu_set_keys"]
    emu_run_frame = exports["emu_run_frame"]
    emu_framebuffer = exports["emu_framebuffer"]
    emu_audio_buffer = exports["emu_audio_buffer"]
    emu_audio_samples = exports["emu_audio_samples"]
    memory = exports["memory"]

    # Init
    result = emu_init(store)
    assert result == 1, f"emu_init failed: {result}"

    # Load ROM
    rom_data = open(rom_path, 'rb').read()
    rom_ptr = emu_rom_buffer(store)
    mem_data = memory.data_ptr(store)
    mem_len = memory.data_len(store)

    import ctypes
    buf = (ctypes.c_ubyte * mem_len).from_address(ctypes.addressof(ctypes.c_ubyte.from_address(mem_data)))
    for i, b in enumerate(rom_data):
        buf[rom_ptr + i] = b

    result = emu_load_rom(store, len(rom_data))
    assert result == 1, f"emu_load_rom failed: {result}"

    # Parse replay
    key_events = {}
    if replay:
        with open(replay) as f:
            for line in f:
                parts = line.strip().split()
                if len(parts) >= 2:
                    frame_num = int(parts[0])
                    keys = int(parts[1], 16)
                    key_events[frame_num] = keys

    # Run frames
    framebuffers = []
    for f in range(frames):
        if f in key_events:
            emu_set_keys(store, key_events[f])

        emu_run_frame(store)

        fb_ptr = emu_framebuffer(store)
        mem_data = memory.data_ptr(store)
        mem_len = memory.data_len(store)
        buf = (ctypes.c_ubyte * mem_len).from_address(ctypes.addressof(ctypes.c_ubyte.from_address(mem_data)))

        # Read 240*160*4 bytes as ABGR
        fb_bytes = bytes(buf[fb_ptr:fb_ptr + 240*160*4])
        framebuffers.append(fb_bytes)

    return framebuffers

def fb_to_rgb(fb_bytes):
    """Convert AABBGGRR framebuffer to RGB numpy array."""
    pixels = np.frombuffer(fb_bytes, dtype=np.uint32).reshape(160, 240)
    r = (pixels & 0xFF).astype(np.uint8)
    g = ((pixels >> 8) & 0xFF).astype(np.uint8)
    b = ((pixels >> 16) & 0xFF).astype(np.uint8)
    return np.stack([r, g, b], axis=2)

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
    frames = int(sys.argv[2]) if len(sys.argv) > 2 else 5
    replay = sys.argv[3] if len(sys.argv) > 3 else None

    print(f"Testing {rom} for {frames} frames")

    # Get oracle reference
    ref_dir = "/tmp/oracle_ref"
    os.makedirs(ref_dir, exist_ok=True)
    cmd = ["oracle", "run", rom, str(frames), "--dump-frames", ref_dir]
    if replay:
        cmd += ["--replay", replay]
    result = subprocess.run(cmd, capture_output=True, text=True)
    print(f"Oracle: {result.stdout.strip()}")

    # Run emulator
    print("Running emulator...")
    try:
        emu_fbs = run_emulator(rom, frames, replay)
    except Exception as e:
        print(f"Emulator error: {e}")
        import traceback
        traceback.print_exc()
        return

    # Compare
    print(f"\nComparing {frames} frames:")
    total_match = 0
    for i in range(frames):
        ref_path = f"{ref_dir}/frame_{i:05d}.ppm"
        if not os.path.exists(ref_path):
            print(f"  Frame {i}: reference missing")
            continue

        ref_rgb = read_ppm(ref_path)
        emu_rgb = fb_to_rgb(emu_fbs[i])

        diff = np.abs(ref_rgb.astype(int) - emu_rgb.astype(int))
        if diff.sum() == 0:
            print(f"  Frame {i}: PERFECT MATCH")
            total_match += 1
        else:
            pct = (diff > 0).sum() / (240 * 160 * 3) * 100
            print(f"  Frame {i}: {pct:.1f}% pixels differ, max={diff.max()}, mean={diff[diff>0].mean():.1f}")

    print(f"\n{total_match}/{frames} frames match perfectly")

if __name__ == "__main__":
    main()
