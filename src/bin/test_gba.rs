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

        let mut all_audio: Vec<i16> = Vec::new();

        for f in 0..frames {
            if let Some(&keys) = key_events.get(&f) {
                gba_emu::emu_set_keys(keys);
            }

            gba_emu::emu_run_frame();

            let audio_buf = gba_emu::emu_audio_buffer();
            let audio_samples = gba_emu::emu_audio_samples();
            if audio_samples > 0 {
                let audio_slice = std::slice::from_raw_parts(audio_buf, audio_samples as usize * 2);
                all_audio.extend_from_slice(audio_slice);
            }

            let fb = gba_emu::emu_framebuffer();
            let fb_slice = std::slice::from_raw_parts(fb, 240 * 160);

            let gba = &*gba_emu::GBA;
            let pc = gba.cpu.regs[15];
            {
                let dispcnt = gba.bus.ppu.dispcnt;
                let mode = dispcnt & 7;
                let bg_en = (dispcnt >> 8) & 0x1F;
                let thumb = (gba.cpu.cpsr >> 5) & 1;
                eprintln!("Frame {}: PC=0x{:08X} CPSR=0x{:08X} T={} DISPCNT=0x{:04X} mode={} bg={:04b} cycles={} instrs={} ewram_r={}({}) ewram_w={}({}) rom_r={} iwram_r={}",
                    f, pc, gba.cpu.cpsr, thumb, dispcnt, mode, bg_en,
                    gba.bus.total_cycles, gba.bus.debug_instrs_frame,
                    gba.bus.debug_ewram_reads, gba.bus.debug_ewram_reads32,
                    gba.bus.debug_ewram_writes, gba.bus.debug_ewram_writes32,
                    gba.bus.debug_rom_reads, gba.bus.debug_iwram_reads);
            }
            let gba = &mut *gba_emu::GBA;
            gba.bus.debug_instrs_frame = 0;
            gba.bus.debug_ewram_reads = 0;
            gba.bus.debug_ewram_writes = 0;
            gba.bus.debug_ewram_reads32 = 0;
            gba.bus.debug_ewram_writes32 = 0;
            gba.bus.debug_rom_reads = 0;
            gba.bus.debug_iwram_reads = 0;

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

        // Write audio WAV
        let audio_path = format!("{}/audio.wav", output_dir);
        let num_samples = all_audio.len() as u32;
        let sample_rate = gba_emu::emu_audio_rate() as u32;
        let channels = 2u16;
        let bits_per_sample = 16u16;
        let byte_rate = sample_rate * channels as u32 * (bits_per_sample / 8) as u32;
        let block_align = channels * (bits_per_sample / 8);
        let data_size = num_samples * 2;
        let file_size = 36 + data_size;

        let mut wav = Vec::with_capacity(44 + data_size as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&file_size.to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&bits_per_sample.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        for &s in &all_audio {
            wav.extend_from_slice(&s.to_le_bytes());
        }
        std::fs::write(&audio_path, &wav).unwrap();
        eprintln!("Audio: {} stereo pairs @ {} Hz", num_samples / 2, sample_rate);
    }

    eprintln!("Wrote {} frames to {}", frames, output_dir);
}
