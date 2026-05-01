#![no_std]

extern crate alloc;

mod allocator;
mod arm;
mod apu;
mod bus;
mod cpu;
mod dma;
mod ppu;
mod thumb;
mod timer;

use alloc::boxed::Box;
use core::ptr;

use bus::Bus;
use cpu::Cpu;

pub struct Gba {
    cpu: Cpu,
    bus: Bus,
}

static mut GBA: *mut Gba = ptr::null_mut();
static mut ROM_BUFFER: *mut u8 = ptr::null_mut();
static mut ROM_BUFFER_LEN: usize = 0;

const ROM_BUFFER_SIZE: usize = 32 * 1024 * 1024;

#[no_mangle]
pub extern "C" fn emu_init() -> i32 {
    unsafe {
        if GBA.is_null() {
            let rom_buf = alloc::vec![0u8; ROM_BUFFER_SIZE];
            ROM_BUFFER = rom_buf.as_ptr() as *mut u8;
            ROM_BUFFER_LEN = ROM_BUFFER_SIZE;
            core::mem::forget(rom_buf);

            let gba = Box::new(Gba {
                cpu: Cpu::new(),
                bus: Bus::new(),
            });
            GBA = Box::into_raw(gba);
        }
        1
    }
}

#[no_mangle]
pub extern "C" fn emu_rom_buffer() -> *mut u8 {
    unsafe { ROM_BUFFER }
}

#[no_mangle]
pub extern "C" fn emu_load_rom(len: i32) -> i32 {
    unsafe {
        if GBA.is_null() || ROM_BUFFER.is_null() {
            return 0;
        }
        let gba = &mut *GBA;
        let rom_data = core::slice::from_raw_parts(ROM_BUFFER, len as usize);
        gba.bus.load_rom(rom_data);
        gba.cpu = Cpu::new();
        gba.bus.reset();
        gba.cpu.reset(&gba.bus);
        1
    }
}

#[no_mangle]
pub extern "C" fn emu_reset() -> i32 {
    unsafe {
        if GBA.is_null() {
            return 0;
        }
        let gba = &mut *GBA;
        gba.bus.reset();
        gba.cpu = Cpu::new();
        gba.cpu.reset(&gba.bus);
        1
    }
}

#[no_mangle]
pub extern "C" fn emu_set_keys(k: u32) {
    unsafe {
        if !GBA.is_null() {
            let gba = &mut *GBA;
            gba.bus.set_keys(k as u16);
        }
    }
}

#[no_mangle]
pub extern "C" fn emu_run_frame() {
    unsafe {
        if !GBA.is_null() {
            let gba = &mut *GBA;
            gba.run_frame();
        }
    }
}

#[no_mangle]
pub extern "C" fn emu_framebuffer() -> *mut u32 {
    unsafe {
        if !GBA.is_null() {
            let gba = &mut *GBA;
            gba.bus.framebuffer.as_mut_ptr()
        } else {
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn emu_audio_buffer() -> *mut i16 {
    unsafe {
        if !GBA.is_null() {
            let gba = &mut *GBA;
            gba.bus.audio_output_buffer.as_mut_ptr()
        } else {
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn emu_audio_samples() -> i32 {
    unsafe {
        if !GBA.is_null() {
            let gba = &mut *GBA;
            let samples = gba.bus.audio_samples_ready;
            gba.bus.audio_samples_ready = 0;
            samples as i32
        } else {
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn emu_audio_rate() -> i32 {
    32768
}

impl Gba {
    pub fn run_frame(&mut self) {
        let start_frame = self.bus.frame_count;
        while self.bus.frame_count == start_frame {
            self.step();
        }
    }

    fn step(&mut self) {
        if self.bus.dma_active() {
            let dma_cycles = self.bus.run_dma();
            self.bus.tick(dma_cycles, &mut self.cpu);
            return;
        }

        if self.bus.halted {
            let remaining = 1232u32.saturating_sub(self.bus.scanline_cycles);
            let advance = if remaining > 0 { remaining } else { 1 };
            self.bus.tick(advance, &mut self.cpu);
            return;
        }

        let cycles = self.cpu.step(&mut self.bus);
        self.bus.tick(cycles, &mut self.cpu);
    }
}
