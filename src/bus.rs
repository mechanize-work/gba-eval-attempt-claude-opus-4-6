use alloc::vec;
use alloc::vec::Vec;
use crate::cpu::Cpu;
use crate::ppu::Ppu;
use crate::apu::Apu;
use crate::dma::{DmaController, DmaTrigger};
use crate::timer::Timers;

const BIOS: &[u8] = include_bytes!("../spec/gba_bios_stub.bin");

pub struct Bus {
    pub ewram: Vec<u8>,
    pub iwram: Vec<u8>,
    pub palette: Vec<u8>,
    pub vram: Vec<u8>,
    pub oam: Vec<u8>,
    pub rom: Vec<u8>,
    pub sram: Vec<u8>,

    pub ppu: Ppu,
    pub apu: Apu,
    pub dma: DmaController,
    pub timers: Timers,

    pub keyinput: u16,

    pub ime: bool,
    pub ie: u16,
    pub if_: u16,

    pub cycles: u32,
    pub scanline_cycles: u32,
    #[cfg(feature = "native-test")]
    pub total_cycles: u64,
    pub current_scanline: u16,
    pub frame_count: u64,

    pub framebuffer: Vec<u32>,
    pub audio_output_buffer: Vec<i16>,
    pub audio_samples_ready: usize,
    pub audio_cycles: u32,

    pub halted: bool,
    pub post_boot: u8,
    pub waitcnt: u16,
    pub bios_latch: u32,

    pub io_regs: Vec<u8>,

    pub ws_n: [u32; 3],
    pub ws_s: [u32; 3],
    pub data_wait_cycles: u32,
    pub fetching_code: bool,
    pub prefetch: bool,
    pub last_rom_data_addr: u32,
    pub prev_exec_cycles: u32,
    pub prev_was_branch: bool,
    pub write_wait_cycles: u32,
}

impl Bus {
    pub fn new() -> Self {
        Self {
            ewram: vec![0u8; 256 * 1024],
            iwram: vec![0u8; 32 * 1024],
            palette: vec![0u8; 1024],
            vram: vec![0u8; 96 * 1024],
            oam: vec![0u8; 1024],
            rom: Vec::new(),
            sram: vec![0u8; 64 * 1024],

            ppu: Ppu::new(),
            apu: Apu::new(),
            dma: DmaController::new(),
            timers: Timers::new(),

            keyinput: 0x03FF,

            ime: false,
            ie: 0,
            if_: 0,

            cycles: 0,
            scanline_cycles: 0,
            #[cfg(feature = "native-test")]
            total_cycles: 0,
            current_scanline: 0,
            frame_count: 0,

            framebuffer: vec![0u32; 240 * 160],
            audio_output_buffer: vec![0i16; 65536],
            audio_samples_ready: 0,
            audio_cycles: 0,

            halted: false,
            post_boot: 0,
            waitcnt: 0,
            bios_latch: 0,

            io_regs: vec![0u8; 0x400],

            ws_n: [5, 5, 5],
            ws_s: [3, 5, 9],
            data_wait_cycles: 0,
            fetching_code: false,
            prefetch: false,
            last_rom_data_addr: 0xFFFF_FFFF,
            prev_exec_cycles: 0,
            prev_was_branch: true,
            write_wait_cycles: 0,
        }
    }

    fn update_waitcnt(&mut self) {
        const N_LUT: [u32; 4] = [4, 3, 2, 8];
        const S0_LUT: [u32; 2] = [2, 1];
        const S1_LUT: [u32; 2] = [4, 1];
        const S2_LUT: [u32; 2] = [8, 1];
        let w = self.waitcnt as usize;
        self.ws_n[0] = 1 + N_LUT[(w >> 2) & 3];
        self.ws_s[0] = 1 + S0_LUT[(w >> 4) & 1];
        self.ws_n[1] = 1 + N_LUT[(w >> 5) & 3];
        self.ws_s[1] = 1 + S1_LUT[(w >> 7) & 1];
        self.ws_n[2] = 1 + N_LUT[(w >> 8) & 3];
        self.ws_s[2] = 1 + S2_LUT[(w >> 10) & 1];
    }

    fn add_data_wait(&mut self, addr: u32, size: u32) {
        if self.fetching_code {
            return;
        }
        let region = (addr >> 24) & 0xF;
        match region {
            0x08 | 0x09 | 0x0A | 0x0B | 0x0C | 0x0D => {
                let ws_idx = match region {
                    0x08 | 0x09 => 0,
                    0x0A | 0x0B => 1,
                    _ => 2,
                };
                let sequential = addr == self.last_rom_data_addr.wrapping_add(4)
                    || addr == self.last_rom_data_addr.wrapping_add(2);
                self.last_rom_data_addr = addr;
                if size == 4 {
                    if sequential {
                        self.data_wait_cycles += self.ws_s[ws_idx] + self.ws_s[ws_idx] - 1;
                    } else {
                        self.data_wait_cycles += self.ws_n[ws_idx] + self.ws_s[ws_idx] - 1;
                    }
                } else {
                    if sequential {
                        self.data_wait_cycles += self.ws_s[ws_idx] - 1;
                    } else {
                        self.data_wait_cycles += self.ws_n[ws_idx] - 1;
                    }
                }
            }
            0x02 => {
                self.last_rom_data_addr = 0xFFFF_FFFF;
            }
            _ => {
                self.last_rom_data_addr = 0xFFFF_FFFF;
            }
        }
    }


    pub fn load_rom(&mut self, data: &[u8]) {
        self.rom = vec![0u8; data.len().max(1)];
        self.rom[..data.len()].copy_from_slice(data);
    }

    pub fn reset(&mut self) {
        self.ewram.fill(0);
        self.iwram.fill(0);
        self.palette.fill(0);
        self.vram.fill(0);
        self.oam.fill(0);
        self.sram.fill(0);
        self.io_regs.fill(0);

        self.ppu = Ppu::new();
        self.apu = Apu::new();
        self.dma = DmaController::new();
        self.timers = Timers::new();

        self.keyinput = 0x03FF;
        self.ime = false;
        self.ie = 0;
        self.if_ = 0;
        self.cycles = 0;
        self.scanline_cycles = 0;
        #[cfg(feature = "native-test")]
        { self.total_cycles = 0; }
        self.current_scanline = 0;
        self.frame_count = 0;
        self.audio_samples_ready = 0;
        self.audio_cycles = 0;
        self.halted = false;
        self.post_boot = 0;
        self.waitcnt = 0;
        self.bios_latch = 0;
        self.ws_n = [5, 5, 5];
        self.ws_s = [3, 5, 9];
        self.prefetch = false;
    }

    pub fn set_keys(&mut self, keys: u16) {
        self.keyinput = !keys & 0x03FF;
    }

    pub fn read8(&mut self, addr: u32) -> u8 {
        self.add_data_wait(addr, 1);
        let region = (addr >> 24) & 0xFF;
        match region {
            0x00 => {
                if addr < 0x4000 {
                    let val = BIOS[addr as usize & 0x3FFF];
                    self.bios_latch = (self.bios_latch & 0xFFFFFF00) | val as u32;
                    val
                } else {
                    (self.bios_latch >> ((addr & 3) * 8)) as u8
                }
            }
            0x02 => self.ewram[(addr & 0x3FFFF) as usize],
            0x03 => self.iwram[(addr & 0x7FFF) as usize],
            0x04 => self.io_read8(addr & 0xFFF),
            0x05 => self.palette[(addr & 0x3FF) as usize],
            0x06 => {
                let a = vram_mirror(addr);
                self.vram[a]
            }
            0x07 => self.oam[(addr & 0x3FF) as usize],
            0x08..=0x0D => {
                let offset = (addr & 0x01FF_FFFF) as usize;
                if offset < self.rom.len() { self.rom[offset] } else { 0 }
            }
            0x0E | 0x0F => {
                self.sram[(addr & 0xFFFF) as usize]
            }
            _ => 0,
        }
    }

    pub fn read16(&mut self, addr: u32) -> u16 {
        let addr = addr & !1;
        self.add_data_wait(addr, 2);
        let region = (addr >> 24) & 0xFF;
        match region {
            0x00 => {
                if addr < 0x4000 {
                    let idx = (addr & 0x3FFF) as usize;
                    let val = u16::from_le_bytes([BIOS[idx], BIOS[idx + 1]]);
                    self.bios_latch = val as u32 | ((val as u32) << 16);
                    val
                } else {
                    (self.bios_latch >> ((addr & 2) * 8)) as u16
                }
            }
            0x02 => {
                let idx = (addr & 0x3FFFF) as usize;
                u16::from_le_bytes([self.ewram[idx], self.ewram[idx + 1]])
            }
            0x03 => {
                let idx = (addr & 0x7FFF) as usize;
                u16::from_le_bytes([self.iwram[idx], self.iwram[idx + 1]])
            }
            0x04 => self.io_read16(addr & 0xFFF),
            0x05 => {
                let idx = (addr & 0x3FF) as usize;
                u16::from_le_bytes([self.palette[idx], self.palette[idx + 1]])
            }
            0x06 => {
                let a = vram_mirror(addr);
                u16::from_le_bytes([self.vram[a], self.vram[a + 1]])
            }
            0x07 => {
                let idx = (addr & 0x3FF) as usize;
                u16::from_le_bytes([self.oam[idx], self.oam[idx + 1]])
            }
            0x08..=0x0D => {
                let offset = (addr & 0x01FF_FFFF) as usize;
                if offset + 1 < self.rom.len() {
                    u16::from_le_bytes([self.rom[offset], self.rom[offset + 1]])
                } else {
                    0
                }
            }
            0x0E | 0x0F => {
                let b = self.sram[(addr & 0xFFFF) as usize];
                u16::from_le_bytes([b, b])
            }
            _ => 0,
        }
    }

    pub fn read32(&mut self, addr: u32) -> u32 {
        let addr = addr & !3;
        self.add_data_wait(addr, 4);
        let region = (addr >> 24) & 0xFF;
        match region {
            0x00 => {
                if addr < 0x4000 {
                    let idx = (addr & 0x3FFF) as usize;
                    let val = u32::from_le_bytes([BIOS[idx], BIOS[idx+1], BIOS[idx+2], BIOS[idx+3]]);
                    self.bios_latch = val;
                    val
                } else {
                    self.bios_latch
                }
            }
            0x02 => {
                let idx = (addr & 0x3FFFF) as usize;
                u32::from_le_bytes([self.ewram[idx], self.ewram[idx+1], self.ewram[idx+2], self.ewram[idx+3]])
            }
            0x03 => {
                let idx = (addr & 0x7FFF) as usize;
                u32::from_le_bytes([self.iwram[idx], self.iwram[idx+1], self.iwram[idx+2], self.iwram[idx+3]])
            }
            0x04 => {
                let lo = self.io_read16(addr & 0xFFF) as u32;
                let hi = self.io_read16((addr & 0xFFF) + 2) as u32;
                lo | (hi << 16)
            }
            0x05 => {
                let idx = (addr & 0x3FF) as usize;
                u32::from_le_bytes([self.palette[idx], self.palette[idx+1], self.palette[idx+2], self.palette[idx+3]])
            }
            0x06 => {
                let a = vram_mirror(addr);
                u32::from_le_bytes([self.vram[a], self.vram[a+1], self.vram[a+2], self.vram[a+3]])
            }
            0x07 => {
                let idx = (addr & 0x3FF) as usize;
                u32::from_le_bytes([self.oam[idx], self.oam[idx+1], self.oam[idx+2], self.oam[idx+3]])
            }
            0x08..=0x0D => {
                let offset = (addr & 0x01FF_FFFF) as usize;
                if offset + 3 < self.rom.len() {
                    u32::from_le_bytes([self.rom[offset], self.rom[offset+1], self.rom[offset+2], self.rom[offset+3]])
                } else {
                    0
                }
            }
            0x0E | 0x0F => {
                let b = self.sram[(addr & 0xFFFF) as usize];
                u32::from_le_bytes([b, b, b, b])
            }
            _ => 0,
        }
    }

    pub fn write8(&mut self, addr: u32, val: u8) {
        self.add_data_wait(addr, 1);
        let region = (addr >> 24) & 0xFF;
        match region {
            0x02 => self.ewram[(addr & 0x3FFFF) as usize] = val,
            0x03 => self.iwram[(addr & 0x7FFF) as usize] = val,
            0x04 => self.io_write8(addr & 0xFFF, val),
            0x05 => {
                let idx = (addr & 0x3FE) as usize;
                self.palette[idx] = val;
                self.palette[idx + 1] = val;
            }
            0x06 => {
                let a = vram_mirror(addr) & !1;
                self.vram[a] = val;
                self.vram[a + 1] = val;
            }
            0x0E | 0x0F => {
                self.sram[(addr & 0xFFFF) as usize] = val;
            }
            _ => {}
        }
    }

    pub fn write16(&mut self, addr: u32, val: u16) {
        let addr = addr & !1;
        self.add_data_wait(addr, 2);
        self.add_write_wait(addr, 2);
        let region = (addr >> 24) & 0xFF;
        let bytes = val.to_le_bytes();
        match region {
            0x02 => {
                let idx = (addr & 0x3FFFF) as usize;
                self.ewram[idx] = bytes[0];
                self.ewram[idx + 1] = bytes[1];
            }
            0x03 => {
                let idx = (addr & 0x7FFF) as usize;
                self.iwram[idx] = bytes[0];
                self.iwram[idx + 1] = bytes[1];
            }
            0x04 => self.io_write16(addr & 0xFFF, val),
            0x05 => {
                let idx = (addr & 0x3FF) as usize;
                self.palette[idx] = bytes[0];
                self.palette[idx + 1] = bytes[1];
            }
            0x06 => {
                let a = vram_mirror(addr);
                self.vram[a] = bytes[0];
                self.vram[a + 1] = bytes[1];
            }
            0x07 => {
                let idx = (addr & 0x3FF) as usize;
                self.oam[idx] = bytes[0];
                self.oam[idx + 1] = bytes[1];
            }
            0x08..=0x0D => {}
            0x0E | 0x0F => {
                self.sram[(addr & 0xFFFF) as usize] = bytes[0];
            }
            _ => {}
        }
    }

    fn add_write_wait(&mut self, _addr: u32, _size: u32) {
    }

    pub fn write32(&mut self, addr: u32, val: u32) {
        let addr = addr & !3;
        self.add_data_wait(addr, 4);
        self.add_write_wait(addr, 4);
        let region = (addr >> 24) & 0xFF;
        let bytes = val.to_le_bytes();
        match region {
            0x02 => {
                let idx = (addr & 0x3FFFF) as usize;
                self.ewram[idx..idx+4].copy_from_slice(&bytes);
            }
            0x03 => {
                let idx = (addr & 0x7FFF) as usize;
                self.iwram[idx..idx+4].copy_from_slice(&bytes);
            }
            0x04 => {
                self.io_write16(addr & 0xFFF, val as u16);
                self.io_write16((addr & 0xFFF) + 2, (val >> 16) as u16);
            }
            0x05 => {
                let idx = (addr & 0x3FF) as usize;
                self.palette[idx..idx+4].copy_from_slice(&bytes);
            }
            0x06 => {
                let a = vram_mirror(addr);
                self.vram[a..a+4].copy_from_slice(&bytes);
            }
            0x07 => {
                let idx = (addr & 0x3FF) as usize;
                self.oam[idx..idx+4].copy_from_slice(&bytes);
            }
            0x08..=0x0D => {}
            0x0E | 0x0F => {
                self.sram[(addr & 0xFFFF) as usize] = bytes[0];
            }
            _ => {}
        }
    }

    pub fn io_read8(&self, addr: u32) -> u8 {
        let val16 = self.io_read16(addr & !1);
        if addr & 1 == 0 { val16 as u8 } else { (val16 >> 8) as u8 }
    }

    pub fn io_read16(&self, addr: u32) -> u16 {
        match addr {
            0x000 => self.ppu.dispcnt,
            0x002 => self.ppu.green_swap,
            0x004 => self.ppu.dispstat,
            0x006 => self.current_scanline as u16,
            0x008 => self.ppu.bgcnt[0],
            0x00A => self.ppu.bgcnt[1],
            0x00C => self.ppu.bgcnt[2],
            0x00E => self.ppu.bgcnt[3],
            0x010 => self.ppu.bghofs[0],
            0x012 => self.ppu.bgvofs[0],
            0x014 => self.ppu.bghofs[1],
            0x016 => self.ppu.bgvofs[1],
            0x018 => self.ppu.bghofs[2],
            0x01A => self.ppu.bgvofs[2],
            0x01C => self.ppu.bghofs[3],
            0x01E => self.ppu.bgvofs[3],
            0x040 => self.ppu.win0h,
            0x042 => self.ppu.win1h,
            0x044 => self.ppu.win0v,
            0x046 => self.ppu.win1v,
            0x048 => self.ppu.winin,
            0x04A => self.ppu.winout,
            0x04C => self.ppu.mosaic,
            0x050 => self.ppu.bldcnt,
            0x052 => self.ppu.bldalpha,
            0x054 => self.ppu.bldy,

            0x060 => self.apu.read_reg(addr),
            0x062 => self.apu.read_reg(addr),
            0x064 => self.apu.read_reg(addr),
            0x068 => self.apu.read_reg(addr),
            0x06C => self.apu.read_reg(addr),
            0x070 => self.apu.read_reg(addr),
            0x072 => self.apu.read_reg(addr),
            0x074 => self.apu.read_reg(addr),
            0x078 => self.apu.read_reg(addr),
            0x07C => self.apu.read_reg(addr),
            0x080 => self.apu.read_reg(addr),
            0x082 => self.apu.read_reg(addr),
            0x084 => self.apu.read_reg(addr),
            0x088 => self.apu.read_reg(addr),
            0x090..=0x09E => self.apu.read_reg(addr),

            0x0B0..=0x0DE => self.dma.read_reg(addr),

            0x100 => self.timers.read_counter(0),
            0x102 => self.timers.read_control(0),
            0x104 => self.timers.read_counter(1),
            0x106 => self.timers.read_control(1),
            0x108 => self.timers.read_counter(2),
            0x10A => self.timers.read_control(2),
            0x10C => self.timers.read_counter(3),
            0x10E => self.timers.read_control(3),

            0x130 => self.keyinput,
            0x132 => self.io_regs[0x132] as u16 | ((self.io_regs[0x133] as u16) << 8),

            0x200 => self.ie,
            0x202 => self.if_,
            0x204 => self.waitcnt,
            0x208 => if self.ime { 1 } else { 0 },

            0x300 => self.post_boot as u16,

            _ => {
                let idx = (addr & 0x3FE) as usize;
                if idx + 1 < self.io_regs.len() {
                    u16::from_le_bytes([self.io_regs[idx], self.io_regs[idx + 1]])
                } else {
                    0
                }
            }
        }
    }

    pub fn io_write8(&mut self, addr: u32, val: u8) {
        if addr == 0x301 {
            self.halted = true;
            return;
        }
        let aligned = addr & !1;
        let old = self.io_read16(aligned);
        let new_val = if addr & 1 == 0 {
            (old & 0xFF00) | val as u16
        } else {
            (old & 0x00FF) | ((val as u16) << 8)
        };
        self.io_write16(aligned, new_val);
    }

    pub fn io_write16(&mut self, addr: u32, val: u16) {
        match addr {
            0x000 => {
                #[cfg(feature = "native-test")]
                {
                    let old_blank = self.ppu.dispcnt & 0x80 != 0;
                    let new_blank = val & 0x80 != 0;
                    if old_blank != new_blank || val != self.ppu.dispcnt {
                        eprintln!("  DISPCNT changed: 0x{:04X} -> 0x{:04X} at scanline={} cycle={} frame={} forced_blank: {} -> {}",
                            self.ppu.dispcnt, val, self.current_scanline, self.scanline_cycles, self.frame_count,
                            old_blank, new_blank);
                    }
                }
                self.ppu.dispcnt = val;
            }
            0x002 => self.ppu.green_swap = val,
            0x004 => {
                self.ppu.dispstat = (self.ppu.dispstat & 0x7) | (val & !0x7);
            }
            0x008 => self.ppu.bgcnt[0] = val,
            0x00A => self.ppu.bgcnt[1] = val,
            0x00C => self.ppu.bgcnt[2] = val,
            0x00E => self.ppu.bgcnt[3] = val,
            0x010 => self.ppu.bghofs[0] = val & 0x1FF,
            0x012 => self.ppu.bgvofs[0] = val & 0x1FF,
            0x014 => self.ppu.bghofs[1] = val & 0x1FF,
            0x016 => self.ppu.bgvofs[1] = val & 0x1FF,
            0x018 => self.ppu.bghofs[2] = val & 0x1FF,
            0x01A => self.ppu.bgvofs[2] = val & 0x1FF,
            0x01C => self.ppu.bghofs[3] = val & 0x1FF,
            0x01E => self.ppu.bgvofs[3] = val & 0x1FF,

            0x020 => self.ppu.bg_affine[0].pa = val as i16,
            0x022 => self.ppu.bg_affine[0].pb = val as i16,
            0x024 => self.ppu.bg_affine[0].pc = val as i16,
            0x026 => self.ppu.bg_affine[0].pd = val as i16,
            0x028 => {
                self.ppu.bg_affine[0].ref_x = (self.ppu.bg_affine[0].ref_x & 0xFFFF0000) | val as u32;
                self.ppu.bg_affine[0].internal_x = sign_extend_28(self.ppu.bg_affine[0].ref_x);
            }
            0x02A => {
                self.ppu.bg_affine[0].ref_x = (self.ppu.bg_affine[0].ref_x & 0x0000FFFF) | ((val as u32) << 16);
                self.ppu.bg_affine[0].internal_x = sign_extend_28(self.ppu.bg_affine[0].ref_x);
            }
            0x02C => {
                self.ppu.bg_affine[0].ref_y = (self.ppu.bg_affine[0].ref_y & 0xFFFF0000) | val as u32;
                self.ppu.bg_affine[0].internal_y = sign_extend_28(self.ppu.bg_affine[0].ref_y);
            }
            0x02E => {
                self.ppu.bg_affine[0].ref_y = (self.ppu.bg_affine[0].ref_y & 0x0000FFFF) | ((val as u32) << 16);
                self.ppu.bg_affine[0].internal_y = sign_extend_28(self.ppu.bg_affine[0].ref_y);
            }

            0x030 => self.ppu.bg_affine[1].pa = val as i16,
            0x032 => self.ppu.bg_affine[1].pb = val as i16,
            0x034 => self.ppu.bg_affine[1].pc = val as i16,
            0x036 => self.ppu.bg_affine[1].pd = val as i16,
            0x038 => {
                self.ppu.bg_affine[1].ref_x = (self.ppu.bg_affine[1].ref_x & 0xFFFF0000) | val as u32;
                self.ppu.bg_affine[1].internal_x = sign_extend_28(self.ppu.bg_affine[1].ref_x);
            }
            0x03A => {
                self.ppu.bg_affine[1].ref_x = (self.ppu.bg_affine[1].ref_x & 0x0000FFFF) | ((val as u32) << 16);
                self.ppu.bg_affine[1].internal_x = sign_extend_28(self.ppu.bg_affine[1].ref_x);
            }
            0x03C => {
                self.ppu.bg_affine[1].ref_y = (self.ppu.bg_affine[1].ref_y & 0xFFFF0000) | val as u32;
                self.ppu.bg_affine[1].internal_y = sign_extend_28(self.ppu.bg_affine[1].ref_y);
            }
            0x03E => {
                self.ppu.bg_affine[1].ref_y = (self.ppu.bg_affine[1].ref_y & 0x0000FFFF) | ((val as u32) << 16);
                self.ppu.bg_affine[1].internal_y = sign_extend_28(self.ppu.bg_affine[1].ref_y);
            }

            0x040 => self.ppu.win0h = val,
            0x042 => self.ppu.win1h = val,
            0x044 => self.ppu.win0v = val,
            0x046 => self.ppu.win1v = val,
            0x048 => self.ppu.winin = val,
            0x04A => self.ppu.winout = val,
            0x04C => self.ppu.mosaic = val,
            0x050 => self.ppu.bldcnt = val,
            0x052 => self.ppu.bldalpha = val,
            0x054 => self.ppu.bldy = val,

            0x060..=0x0A6 => {
                self.apu.write_reg(addr, val);
            }

            0x0B0..=0x0DE => {
                self.dma.write_reg(addr, val);
                if addr == 0x0BA || addr == 0x0C6 || addr == 0x0D2 || addr == 0x0DE {
                    let ch = ((addr - 0x0BA) / 0xC) as usize;
                    if ch < 4 {
                        self.dma.check_enable(ch);
                    }
                }
            }

            0x100 => self.timers.write_reload(0, val),
            0x102 => self.timers.write_control(0, val),
            0x104 => self.timers.write_reload(1, val),
            0x106 => self.timers.write_control(1, val),
            0x108 => self.timers.write_reload(2, val),
            0x10A => self.timers.write_control(2, val),
            0x10C => self.timers.write_reload(3, val),
            0x10E => self.timers.write_control(3, val),

            0x130 => {}
            0x132 => {
                self.io_regs[0x132] = val as u8;
                self.io_regs[0x133] = (val >> 8) as u8;
            }

            0x200 => self.ie = val,
            0x202 => self.if_ &= !val,
            0x204 => {
                self.waitcnt = val;
                self.update_waitcnt();
                self.prefetch = val & (1 << 14) != 0;
                #[cfg(feature = "native-test")]
                eprintln!("  WAITCNT set to 0x{:04X}: ws0_n={} ws0_s={} ws1_n={} ws1_s={} ws2_n={} ws2_s={} prefetch={}",
                    val, self.ws_n[0], self.ws_s[0], self.ws_n[1], self.ws_s[1],
                    self.ws_n[2], self.ws_s[2], self.prefetch);
            }
            0x208 => self.ime = val & 1 != 0,

            0x300 => {
                self.post_boot = val as u8;
                if val & 0x80 != 0 {
                    self.halted = true;
                }
            }
            0x301 => {
                self.halted = true;
            }

            _ => {
                let idx = (addr & 0x3FE) as usize;
                if idx + 1 < self.io_regs.len() {
                    let bytes = val.to_le_bytes();
                    self.io_regs[idx] = bytes[0];
                    self.io_regs[idx + 1] = bytes[1];
                }
            }
        }
    }

    pub fn dma_active(&self) -> bool {
        self.dma.any_active()
    }

    pub fn run_dma(&mut self) -> u32 {
        let mut total_cycles = 0u32;
        for ch in 0..4 {
            if !self.dma.channels[ch].active {
                continue;
            }
            let c = &mut self.dma.channels[ch];
            let is_32bit = c.cnt & (1 << 10) != 0;
            let src_ctrl = (c.cnt >> 7) & 3;
            let dst_ctrl = (c.cnt >> 5) & 3;

            let mut src = c.internal_src;
            let mut dst = c.internal_dst;
            let mut remaining = c.internal_count;
            let width = if is_32bit { 4u32 } else { 2 };





            while remaining > 0 {
                if is_32bit {
                    let val = self.read32(src);
                    self.write32(dst, val);
                } else {
                    let val = self.read16(src);
                    self.write16(dst, val);
                }

                match src_ctrl {
                    0 => src = src.wrapping_add(width),
                    1 => src = src.wrapping_sub(width),
                    2 => {}
                    _ => src = src.wrapping_add(width),
                }
                match dst_ctrl {
                    0 => dst = dst.wrapping_add(width),
                    1 => dst = dst.wrapping_sub(width),
                    2 => {}
                    3 => dst = dst.wrapping_add(width),
                    _ => {}
                }

                remaining -= 1;
                total_cycles += 2;
            }

            self.dma.channels[ch].internal_src = src;
            self.dma.channels[ch].internal_dst = dst;
            self.dma.channels[ch].internal_count = 0;
            self.dma.channels[ch].active = false;

            let repeat = self.dma.channels[ch].cnt & (1 << 9) != 0;
            let timing = (self.dma.channels[ch].cnt >> 12) & 3;

            if repeat && timing != 0 {
                if dst_ctrl == 3 {
                    self.dma.channels[ch].internal_dst = self.dma.channels[ch].dst;
                }
                self.dma.channels[ch].internal_count = if self.dma.channels[ch].count == 0 {
                    if ch == 3 { 0x10000 } else { 0x4000 }
                } else {
                    self.dma.channels[ch].count as u32
                };
            } else if timing == 0 || !repeat {
                self.dma.channels[ch].cnt &= !(1 << 15);
            }

            if self.dma.channels[ch].cnt & (1 << 14) != 0 {
                self.if_ |= 1 << (8 + ch);
            }

            break;
        }
        if total_cycles == 0 { 1 } else { total_cycles }
    }

    pub fn tick(&mut self, cycles: u32, cpu: &mut Cpu) {
        self.timers.tick(cycles, &mut self.apu, &mut self.if_);

        self.generate_audio(cycles);

        #[cfg(feature = "native-test")]
        { self.total_cycles += cycles as u64; }
        self.scanline_cycles += cycles;
        while self.scanline_cycles >= 1232 {
            self.scanline_cycles -= 1232;
            self.advance_scanline(cpu);
        }

        if self.scanline_cycles >= 960 {
            let was_hblank = self.ppu.dispstat & 0x2 != 0;
            self.ppu.dispstat |= 0x2;
            if !was_hblank && self.current_scanline < 160 {
                if self.ppu.dispstat & (1 << 4) != 0 {
                    self.if_ |= 0x2;
                }
                self.dma.trigger(DmaTrigger::HBlank);
            }
        } else {
            self.ppu.dispstat &= !0x2;
        }

        if self.halted && (self.ie & self.if_) != 0 {
            self.halted = false;
        }
    }

    fn advance_scanline(&mut self, _cpu: &mut Cpu) {
        let old_line = self.current_scanline;

        if old_line < 160 {
            self.ppu.render_scanline(old_line, &self.palette, &self.vram, &self.oam, &mut self.framebuffer);
        }

        self.current_scanline += 1;

        if self.current_scanline >= 228 {
            self.current_scanline = 0;
            self.frame_count += 1;
            self.ppu.on_vblank_end();
        }

        if self.current_scanline == 160 {
            self.ppu.dispstat |= 0x1;
            if self.ppu.dispstat & (1 << 3) != 0 {
                self.if_ |= 0x1;
            }
            self.dma.trigger(DmaTrigger::VBlank);
        }

        if self.current_scanline == 228 || self.current_scanline == 0 {
            self.ppu.dispstat &= !0x1;
        }

        if self.current_scanline > 160 && self.current_scanline < 227 {
            self.ppu.dispstat |= 0x1;
        }

        let lyc = (self.ppu.dispstat >> 8) as u16 | (if self.ppu.dispstat & (1 << 15) != 0 { 256 } else { 0 });
        if self.current_scanline == lyc {
            self.ppu.dispstat |= 0x4;
            if self.ppu.dispstat & (1 << 5) != 0 {
                self.if_ |= 0x4;
            }
        } else {
            self.ppu.dispstat &= !0x4;
        }

        self.ppu.dispstat &= !0x2;
    }

    pub fn pipeline_stall(&self, pc: u32, is_thumb: bool) -> u32 {
        if self.prev_was_branch {
            return 0;
        }
        let region = (pc >> 24) & 0xF;
        match region {
            0x08 | 0x09 | 0x0A | 0x0B | 0x0C | 0x0D => {
                if self.prefetch { return 0; }
                let ws_idx = match region {
                    0x08 | 0x09 => 0,
                    0x0A | 0x0B => 1,
                    _ => 2,
                };
                let fetch_time = if is_thumb {
                    self.ws_s[ws_idx]
                } else {
                    self.ws_s[ws_idx] + self.ws_s[ws_idx]
                };
                if self.prev_exec_cycles >= fetch_time {
                    0
                } else {
                    fetch_time - self.prev_exec_cycles
                }
            }
            _ => 0,
        }
    }

    pub fn rom_seq_fetch_extra(&self, pc: u32, is_thumb: bool) -> u32 {
        let region = (pc >> 24) & 0xF;
        match region {
            0x08 | 0x09 | 0x0A | 0x0B | 0x0C | 0x0D => {
                if self.prefetch { return 0; }
                let ws_idx = match region {
                    0x08 | 0x09 => 0,
                    0x0A | 0x0B => 1,
                    _ => 2,
                };
                if is_thumb {
                    self.ws_s[ws_idx] - 1
                } else {
                    2 * self.ws_s[ws_idx] - 1
                }
            }
            _ => 0,
        }
    }

    pub fn branch_refill_cycles(&self, target: u32, is_thumb: bool) -> u32 {
        let region = (target >> 24) & 0xF;
        match region {
            0x08 | 0x09 | 0x0A | 0x0B | 0x0C | 0x0D => {
                let ws_idx = match region {
                    0x08 | 0x09 => 0,
                    0x0A | 0x0B => 1,
                    _ => 2,
                };
                if is_thumb {
                    2
                } else {
                    4
                }
            }
            _ => 0,
        }
    }

    pub fn code_fetch_extra(&self, pc: u32, is_thumb: bool, is_branch: bool) -> u32 {
        let region = (pc >> 24) & 0xF;
        match region {
            0x08 | 0x09 | 0x0A | 0x0B | 0x0C | 0x0D => {
                let ws_idx = match region {
                    0x08 | 0x09 => 0,
                    0x0A | 0x0B => 1,
                    _ => 2,
                };
                if self.prefetch && !is_branch {
                    return 0;
                }
                if is_thumb {
                    if is_branch {
                        if self.prefetch {
                            self.ws_n[ws_idx] - 1
                        } else {
                            2 * (self.ws_s[ws_idx] - 1) + (self.ws_n[ws_idx] - 1)
                        }
                    } else {
                        self.ws_s[ws_idx] - 1
                    }
                } else {
                    if is_branch {
                        if self.prefetch {
                            (self.ws_n[ws_idx] - 1) + (self.ws_s[ws_idx] - 1)
                        } else {
                            2 * ((self.ws_n[ws_idx] - 1) + (self.ws_s[ws_idx] - 1)) + (self.ws_n[ws_idx] - 1) + (self.ws_s[ws_idx] - 1)
                        }
                    } else {
                        (self.ws_n[ws_idx] - 1) + (self.ws_s[ws_idx] - 1)
                    }
                }
            }
            0x02 => {
                if is_thumb {
                    if is_branch { 6 } else { 2 }
                } else {
                    if is_branch { 16 } else { 4 }
                }
            }
            _ => 0,
        }
    }

    pub fn bios_hle(&mut self, _swi_num: u32, _cpu: &mut Cpu) -> bool {
        false
    }

    fn hle_cpu_fast_set(&mut self, cpu: &mut Cpu) {
        let src = cpu.regs[0];
        let dst = cpu.regs[1];
        let len_mode = cpu.regs[2];
        let count = ((len_mode & 0x1FFFFF) + 7) & !7;
        let fixed = len_mode & (1 << 24) != 0;

        if fixed {
            let val = self.read32(src);
            for i in 0..count {
                self.write32(dst.wrapping_add(i * 4), val);
            }
        } else {
            for i in 0..count {
                let val = self.read32(src.wrapping_add(i * 4));
                self.write32(dst.wrapping_add(i * 4), val);
            }
        }
        cpu.regs[0] = src.wrapping_add(if fixed { 0 } else { count * 4 });
        cpu.regs[1] = dst.wrapping_add(count * 4);
        cpu.regs[3] = self.read32(src.wrapping_add(if fixed { 0 } else { (count - 1) * 4 }));
    }

    fn hle_cpu_set(&mut self, cpu: &mut Cpu) {
        let src = cpu.regs[0];
        let dst = cpu.regs[1];
        let len_mode = cpu.regs[2];
        let count = len_mode & 0x1FFFFF;
        let fixed = len_mode & (1 << 24) != 0;
        let word_mode = len_mode & (1 << 26) != 0;

        if word_mode {
            if fixed {
                let val = self.read32(src);
                for i in 0..count {
                    self.write32(dst.wrapping_add(i * 4), val);
                }
            } else {
                for i in 0..count {
                    let val = self.read32(src.wrapping_add(i * 4));
                    self.write32(dst.wrapping_add(i * 4), val);
                }
            }
            cpu.regs[0] = src.wrapping_add(if fixed { 0 } else { count * 4 });
            cpu.regs[1] = dst.wrapping_add(count * 4);
        } else {
            if fixed {
                let val = self.read16(src);
                for i in 0..count {
                    self.write16(dst.wrapping_add(i * 2), val);
                }
            } else {
                for i in 0..count {
                    let val = self.read16(src.wrapping_add(i * 2));
                    self.write16(dst.wrapping_add(i * 2), val);
                }
            }
            cpu.regs[0] = src.wrapping_add(if fixed { 0 } else { count * 2 });
            cpu.regs[1] = dst.wrapping_add(count * 2);
        }
    }

    fn generate_audio(&mut self, cycles: u32) {
        self.audio_cycles += cycles;
        let cycles_per_sample = 512;
        while self.audio_cycles >= cycles_per_sample {
            self.audio_cycles -= cycles_per_sample;
            let (left, right) = self.apu.generate_sample(&self.timers);
            let idx = self.audio_samples_ready * 2;
            if idx + 1 < self.audio_output_buffer.len() {
                self.audio_output_buffer[idx] = left;
                self.audio_output_buffer[idx + 1] = right;
                self.audio_samples_ready += 1;
            }
        }
    }
}

fn vram_mirror(addr: u32) -> usize {
    let offset = addr & 0x1FFFF;
    if offset >= 0x18000 {
        (offset - 0x8000) as usize
    } else {
        offset as usize
    }
}

fn sign_extend_28(val: u32) -> i32 {
    let val = val & 0x0FFF_FFFF;
    if val & 0x0800_0000 != 0 {
        (val | 0xF000_0000) as i32
    } else {
        val as i32
    }
}
