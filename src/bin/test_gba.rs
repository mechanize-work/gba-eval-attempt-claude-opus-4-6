const TRACE_SIZE: usize = 4096;

struct TraceEntry {
    pc: u32,
    instr: u32,
    cpsr: u32,
    regs: [u32; 16],
    thumb: bool,
}

static mut TRACE_BUF: [Option<TraceEntry>; TRACE_SIZE] = [const { None }; TRACE_SIZE];
static mut TRACE_IDX: usize = 0;

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

        let mut bad_pc_detected = false;

        for f in 0..frames {
            if let Some(&keys) = key_events.get(&f) {
                gba_emu::emu_set_keys(keys);
            }

            let gba = &mut *gba_emu::GBA;
            let start_frame = gba.bus.frame_count;
            while gba.bus.frame_count == start_frame {
                if gba.bus.dma_active() {
                    let dma_cycles = gba.bus.run_dma();
                    gba.bus.tick(dma_cycles, &mut gba.cpu);
                    continue;
                }

                if gba.bus.halted {
                    let remaining = 1232u32.saturating_sub(gba.bus.scanline_cycles);
                    let advance = if remaining > 0 { remaining } else { 1 };
                    gba.bus.tick(advance, &mut gba.cpu);
                    continue;
                }

                let pre_pc = if gba.cpu.pipeline_valid {
                    if gba.cpu.in_thumb() {
                        gba.cpu.regs[15].wrapping_sub(4)
                    } else {
                        gba.cpu.regs[15].wrapping_sub(8)
                    }
                } else {
                    gba.cpu.regs[15]
                };

                let pre_cpsr = gba.cpu.cpsr;
                let pre_thumb = gba.cpu.in_thumb();
                let pre_instr = if gba.cpu.pipeline_valid {
                    if pre_thumb {
                        gba.bus.read16(pre_pc) as u32
                    } else {
                        gba.bus.read32(pre_pc)
                    }
                } else {
                    0
                };

                let entry = TraceEntry {
                    pc: pre_pc,
                    instr: pre_instr,
                    cpsr: pre_cpsr,
                    regs: gba.cpu.regs,
                    thumb: pre_thumb,
                };
                TRACE_BUF[TRACE_IDX % TRACE_SIZE] = Some(entry);
                TRACE_IDX += 1;

                let cycles = gba.cpu.step(&mut gba.bus);
                gba.bus.tick(cycles, &mut gba.cpu);

                let post_pc = if gba.cpu.pipeline_valid {
                    if gba.cpu.in_thumb() { gba.cpu.regs[15].wrapping_sub(4) } else { gba.cpu.regs[15].wrapping_sub(8) }
                } else {
                    gba.cpu.regs[15]
                };
                let post_region = (post_pc >> 24) & 0xFF;
                let in_ewram_arm = post_region == 0x02 && !gba.cpu.in_thumb();
                if !bad_pc_detected && in_ewram_arm {
                    bad_pc_detected = true;
                    eprintln!("=== BAD PC DETECTED at frame {} ===", f);
                    eprintln!("Post-step: PC=0x{:08X} CPSR=0x{:08X}", post_pc, gba.cpu.cpsr);
                    let start = if TRACE_IDX > TRACE_SIZE { TRACE_IDX - TRACE_SIZE } else { 0 };
                    let mut last_was_zero = false;
                    let mut zero_count = 0u32;
                    for ti in start..TRACE_IDX {
                        if let Some(ref e) = TRACE_BUF[ti % TRACE_SIZE] {
                            if e.instr == 0 && last_was_zero {
                                zero_count += 1;
                                if zero_count == 3 {
                                    eprintln!("  ... (skipping zero-instr NOPs)");
                                }
                                if zero_count >= 3 && ti < TRACE_IDX - 10 {
                                    continue;
                                }
                            } else {
                                last_was_zero = e.instr == 0;
                                zero_count = 0;
                            }
                            let mode = if e.thumb { "T" } else { "A" };
                            eprintln!("  [{}] PC=0x{:08X} {} instr=0x{:08X} CPSR=0x{:08X} R0=0x{:08X} R1=0x{:08X} R2=0x{:08X} R3=0x{:08X} SP=0x{:08X} LR=0x{:08X} R15=0x{:08X}",
                                ti, e.pc, mode, e.instr, e.cpsr, e.regs[0], e.regs[1], e.regs[2], e.regs[3], e.regs[13], e.regs[14], e.regs[15]);
                        }
                    }
                    break;
                }
            }

            if bad_pc_detected {
                break;
            }

            let fb = gba_emu::emu_framebuffer();
            let fb_slice = std::slice::from_raw_parts(fb, 240 * 160);

            let gba = &*gba_emu::GBA;
            let pc = gba.cpu.regs[15];
            if f < 5 || f == frames - 1 || (f > 0 && f % 50 == 0) || pc > 0x10000000 {
                let dispcnt = gba.bus.ppu.dispcnt;
                let mode = dispcnt & 7;
                let bg_en = (dispcnt >> 8) & 0x1F;
                eprintln!("Frame {}: PC=0x{:08X} CPSR=0x{:08X} DISPCNT=0x{:04X} mode={} bg={:04b} SP=0x{:08X} LR=0x{:08X} halted={} IME={} IE=0x{:04X} IF=0x{:04X}",
                    f, pc, gba.cpu.cpsr, dispcnt, mode, bg_en,
                    gba.cpu.regs[13], gba.cpu.regs[14],
                    gba.bus.halted, gba.bus.ime, gba.bus.ie, gba.bus.if_);
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
