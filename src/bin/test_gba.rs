fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: test_gba <rom> <frames> <output_dir> [replay]");
        std::process::exit(1);
    }

    let rom_path = &args[1];
    let frames: usize = args[2].parse().unwrap();
    let output_dir = &args[3];
    let replay_path = args.get(4);

    std::fs::create_dir_all(output_dir).unwrap();

    let mut key_events: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
    if let Some(rp) = replay_path {
        if let Ok(content) = std::fs::read_to_string(rp) {
            for line in content.lines() {
                let parts: Vec<&str> = line.trim().split_whitespace().collect();
                if parts.len() >= 2 {
                    let frame_num: usize = parts[0].parse().unwrap_or(0);
                    let keys = u32::from_str_radix(parts[1].trim_start_matches("0x").trim_start_matches("0X"), 16).unwrap_or(0);
                    key_events.insert(frame_num, keys);
                }
            }
        }
    }

    unsafe {
        gba_emu::emu_init();

        let rom_data = std::fs::read(rom_path).unwrap();
        let rom_buf = gba_emu::emu_rom_buffer();
        std::ptr::copy_nonoverlapping(rom_data.as_ptr(), rom_buf, rom_data.len());

        gba_emu::emu_load_rom(rom_data.len() as i32);

        for f in 0..frames {
            if let Some(&keys) = key_events.get(&f) {
                gba_emu::emu_set_keys(keys);
            }

            gba_emu::emu_run_frame();

            let fb = gba_emu::emu_framebuffer();
            let fb_slice = std::slice::from_raw_parts(fb, 240 * 160);

            let gba = &*gba_emu::GBA;
            let pc = gba.cpu.regs[15];
            if f < 5 || f == frames - 1 || (f > 0 && f % 50 == 0) {
                let dispcnt = gba.bus.ppu.dispcnt;
                let mode = dispcnt & 7;
                let bg_en = (dispcnt >> 8) & 0x1F;
                eprintln!("Frame {}: PC=0x{:08X} CPSR=0x{:08X} DISPCNT=0x{:04X} mode={} bg={:04b} SP=0x{:08X} LR=0x{:08X} halted={} IME={} IE=0x{:04X} IF=0x{:04X} cycles={}",
                    f, pc, gba.cpu.cpsr, dispcnt, mode, bg_en,
                    gba.cpu.regs[13], gba.cpu.regs[14],
                    gba.bus.halted, gba.bus.ime, gba.bus.ie, gba.bus.if_,
                    gba.bus.total_cycles);
            }

            // Debug: check specific pixels
            if f == 1 {
                let p0 = fb_slice[0];  // Line 0, pixel 0
                let p82 = fb_slice[82 * 240]; // Line 82, pixel 0
                let p100 = fb_slice[100 * 240]; // Line 100, pixel 0
                eprintln!("  DEBUG frame 1 pixels: line0=0x{:08X} line82=0x{:08X} line100=0x{:08X}", p0, p82, p100);
            }

            let path = format!("{}/frame_{:05}.ppm", output_dir, f);
            let mut ppm = format!("P6\n240 160\n255\n").into_bytes();
            for &pixel in fb_slice {
                let r = (pixel & 0xFF) as u8;
                let g = ((pixel >> 8) & 0xFF) as u8;
                let b = ((pixel >> 16) & 0xFF) as u8;
                ppm.push(r);
                ppm.push(g);
                ppm.push(b);
            }
            std::fs::write(&path, &ppm).unwrap();
        }
    }

    eprintln!("Wrote {} frames to {}", frames, output_dir);
}
