// Native test - compile and run with: cargo test --test compare -- --nocapture
use std::process::Command;
use std::path::Path;

fn read_ppm(path: &str) -> Vec<u8> {
    let data = std::fs::read(path).unwrap();
    // Skip header (P6\nW H\n255\n)
    let mut pos = 0;
    // Skip "P6\n"
    while pos < data.len() && data[pos] != b'\n' { pos += 1; }
    pos += 1;
    // Skip comments
    while pos < data.len() && data[pos] == b'#' {
        while pos < data.len() && data[pos] != b'\n' { pos += 1; }
        pos += 1;
    }
    // Skip "W H\n"
    while pos < data.len() && data[pos] != b'\n' { pos += 1; }
    pos += 1;
    // Skip "255\n"
    while pos < data.len() && data[pos] != b'\n' { pos += 1; }
    pos += 1;
    data[pos..].to_vec()
}

#[test]
fn test_vs_oracle() {
    // This test compiles natively, not as wasm
    // It uses the rlib build of gba_emu
    println!("This test needs to be run as a native binary");
}
