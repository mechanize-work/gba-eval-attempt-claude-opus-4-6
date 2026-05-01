use crate::bus::Bus;

pub const MODE_USR: u32 = 0x10;
pub const MODE_FIQ: u32 = 0x11;
pub const MODE_IRQ: u32 = 0x12;
pub const MODE_SVC: u32 = 0x13;
pub const MODE_ABT: u32 = 0x17;
pub const MODE_UND: u32 = 0x1B;
pub const MODE_SYS: u32 = 0x1F;

pub const N_FLAG: u32 = 1 << 31;
pub const Z_FLAG: u32 = 1 << 30;
pub const C_FLAG: u32 = 1 << 29;
pub const V_FLAG: u32 = 1 << 28;
pub const I_FLAG: u32 = 1 << 7;
pub const F_FLAG: u32 = 1 << 6;
pub const T_FLAG: u32 = 1 << 5;

pub struct Cpu {
    pub regs: [u32; 16],
    pub cpsr: u32,

    pub spsr_fiq: u32,
    pub spsr_irq: u32,
    pub spsr_svc: u32,
    pub spsr_abt: u32,
    pub spsr_und: u32,

    pub r8_14_fiq: [u32; 7],
    pub r8_12_usr: [u32; 5],
    pub r13_14_irq: [u32; 2],
    pub r13_14_svc: [u32; 2],
    pub r13_14_abt: [u32; 2],
    pub r13_14_und: [u32; 2],
    pub r13_14_usr: [u32; 2],

    pub pipeline: [u32; 2],
    pub pipeline_valid: bool,
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            regs: [0; 16],
            cpsr: MODE_SVC | I_FLAG | F_FLAG,
            spsr_fiq: 0,
            spsr_irq: 0,
            spsr_svc: 0,
            spsr_abt: 0,
            spsr_und: 0,
            r8_14_fiq: [0; 7],
            r8_12_usr: [0; 5],
            r13_14_irq: [0; 2],
            r13_14_svc: [0; 2],
            r13_14_abt: [0; 2],
            r13_14_und: [0; 2],
            r13_14_usr: [0; 2],
            pipeline: [0; 2],
            pipeline_valid: false,
        }
    }

    pub fn reset(&mut self, bus: &Bus) {
        self.cpsr = MODE_SVC | I_FLAG | F_FLAG;
        self.regs = [0; 16];
        self.regs[15] = 0;
        self.regs[13] = 0x0300_7FE0;
        self.r13_14_irq[0] = 0x0300_7FA0;
        self.r13_14_svc[0] = 0x0300_7FE0;
        self.r13_14_usr[0] = 0x0300_7F00;
        self.pipeline_valid = false;
        let _ = bus;
    }

    pub fn get_pc(&self) -> u32 {
        self.regs[15]
    }

    pub fn in_thumb(&self) -> bool {
        self.cpsr & T_FLAG != 0
    }

    fn pc_offset(&self) -> u32 {
        if self.in_thumb() { 4 } else { 8 }
    }

    pub fn get_spsr(&self) -> u32 {
        match self.cpsr & 0x1F {
            MODE_FIQ => self.spsr_fiq,
            MODE_IRQ => self.spsr_irq,
            MODE_SVC => self.spsr_svc,
            MODE_ABT => self.spsr_abt,
            MODE_UND => self.spsr_und,
            _ => self.cpsr,
        }
    }

    pub fn set_spsr(&mut self, val: u32) {
        match self.cpsr & 0x1F {
            MODE_FIQ => self.spsr_fiq = val,
            MODE_IRQ => self.spsr_irq = val,
            MODE_SVC => self.spsr_svc = val,
            MODE_ABT => self.spsr_abt = val,
            MODE_UND => self.spsr_und = val,
            _ => {}
        }
    }

    pub fn switch_mode(&mut self, new_mode: u32) {
        let old_mode = self.cpsr & 0x1F;
        if old_mode == new_mode {
            self.cpsr = (self.cpsr & !0x1F) | new_mode;
            return;
        }

        match old_mode {
            MODE_USR | MODE_SYS => {
                self.r13_14_usr[0] = self.regs[13];
                self.r13_14_usr[1] = self.regs[14];
                self.r8_12_usr = [self.regs[8], self.regs[9], self.regs[10], self.regs[11], self.regs[12]];
            }
            MODE_FIQ => {
                self.r8_14_fiq = [
                    self.regs[8], self.regs[9], self.regs[10], self.regs[11],
                    self.regs[12], self.regs[13], self.regs[14],
                ];
            }
            MODE_IRQ => {
                self.r13_14_irq[0] = self.regs[13];
                self.r13_14_irq[1] = self.regs[14];
            }
            MODE_SVC => {
                self.r13_14_svc[0] = self.regs[13];
                self.r13_14_svc[1] = self.regs[14];
            }
            MODE_ABT => {
                self.r13_14_abt[0] = self.regs[13];
                self.r13_14_abt[1] = self.regs[14];
            }
            MODE_UND => {
                self.r13_14_und[0] = self.regs[13];
                self.r13_14_und[1] = self.regs[14];
            }
            _ => {}
        }

        if old_mode == MODE_FIQ && new_mode != MODE_FIQ {
            self.regs[8] = self.r8_12_usr[0];
            self.regs[9] = self.r8_12_usr[1];
            self.regs[10] = self.r8_12_usr[2];
            self.regs[11] = self.r8_12_usr[3];
            self.regs[12] = self.r8_12_usr[4];
        }

        if old_mode != MODE_FIQ && new_mode == MODE_FIQ {
            self.r8_12_usr = [self.regs[8], self.regs[9], self.regs[10], self.regs[11], self.regs[12]];
        }

        match new_mode {
            MODE_USR | MODE_SYS => {
                self.regs[13] = self.r13_14_usr[0];
                self.regs[14] = self.r13_14_usr[1];
            }
            MODE_FIQ => {
                self.regs[8] = self.r8_14_fiq[0];
                self.regs[9] = self.r8_14_fiq[1];
                self.regs[10] = self.r8_14_fiq[2];
                self.regs[11] = self.r8_14_fiq[3];
                self.regs[12] = self.r8_14_fiq[4];
                self.regs[13] = self.r8_14_fiq[5];
                self.regs[14] = self.r8_14_fiq[6];
            }
            MODE_IRQ => {
                self.regs[13] = self.r13_14_irq[0];
                self.regs[14] = self.r13_14_irq[1];
            }
            MODE_SVC => {
                self.regs[13] = self.r13_14_svc[0];
                self.regs[14] = self.r13_14_svc[1];
            }
            MODE_ABT => {
                self.regs[13] = self.r13_14_abt[0];
                self.regs[14] = self.r13_14_abt[1];
            }
            MODE_UND => {
                self.regs[13] = self.r13_14_und[0];
                self.regs[14] = self.r13_14_und[1];
            }
            _ => {}
        }

        self.cpsr = (self.cpsr & !0x1F) | new_mode;
    }

    pub fn check_condition(&self, cond: u32) -> bool {
        match cond {
            0x0 => self.cpsr & Z_FLAG != 0,
            0x1 => self.cpsr & Z_FLAG == 0,
            0x2 => self.cpsr & C_FLAG != 0,
            0x3 => self.cpsr & C_FLAG == 0,
            0x4 => self.cpsr & N_FLAG != 0,
            0x5 => self.cpsr & N_FLAG == 0,
            0x6 => self.cpsr & V_FLAG != 0,
            0x7 => self.cpsr & V_FLAG == 0,
            0x8 => self.cpsr & C_FLAG != 0 && self.cpsr & Z_FLAG == 0,
            0x9 => self.cpsr & C_FLAG == 0 || self.cpsr & Z_FLAG != 0,
            0xA => (self.cpsr & N_FLAG != 0) == (self.cpsr & V_FLAG != 0),
            0xB => (self.cpsr & N_FLAG != 0) != (self.cpsr & V_FLAG != 0),
            0xC => self.cpsr & Z_FLAG == 0 && (self.cpsr & N_FLAG != 0) == (self.cpsr & V_FLAG != 0),
            0xD => self.cpsr & Z_FLAG != 0 || (self.cpsr & N_FLAG != 0) != (self.cpsr & V_FLAG != 0),
            0xE => true,
            0xF => false,
            _ => false,
        }
    }

    pub fn step(&mut self, bus: &mut Bus) -> u32 {
        if !self.pipeline_valid {
            self.flush_pipeline(bus);
        }

        if self.check_irq(bus) {
            return 3;
        }

        if self.in_thumb() {
            self.execute_thumb(bus)
        } else {
            self.execute_arm(bus)
        }
    }

    fn check_irq(&mut self, bus: &mut Bus) -> bool {
        if self.cpsr & I_FLAG != 0 {
            return false;
        }
        if !bus.ime {
            return false;
        }
        if bus.ie & bus.if_ == 0 {
            return false;
        }

        bus.halted = false;

        let old_cpsr = self.cpsr;
        self.switch_mode(MODE_IRQ);
        self.spsr_irq = old_cpsr;

        let ret_addr = if old_cpsr & T_FLAG != 0 {
            self.regs[15]
        } else {
            self.regs[15].wrapping_sub(4)
        };
        self.regs[14] = ret_addr;

        self.cpsr &= !T_FLAG;
        self.cpsr |= I_FLAG;

        self.regs[15] = 0x18;
        self.pipeline_valid = false;
        true
    }

    pub fn flush_pipeline(&mut self, bus: &mut Bus) {
        let _ = bus;
        if self.in_thumb() {
            self.regs[15] = self.regs[15].wrapping_add(4);
        } else {
            self.regs[15] = self.regs[15].wrapping_add(8);
        }
        self.pipeline_valid = true;
    }

    pub fn execute_arm(&mut self, bus: &mut Bus) -> u32 {
        let instr_addr = self.regs[15].wrapping_sub(8);
        let stall = bus.pipeline_stall(instr_addr, false);
        bus.fetching_code = true;
        let instr = bus.read32(instr_addr);
        bus.fetching_code = false;
        bus.data_wait_cycles = 0;
        bus.write_wait_cycles = 0;
        bus.rom_data_accessed = false;
        bus.last_rom_data_addr = 0xFFFF_FFFF;

        let cond = (instr >> 28) & 0xF;
        if !self.check_condition(cond) {
            self.regs[15] = self.regs[15].wrapping_add(4);
            let total = 1 + stall;
            bus.prev_exec_cycles = 1;
            bus.prev_was_branch = false;
            return total;
        }

        self.pipeline_valid = true;
        let cycles = crate::arm::execute(self, bus, instr);

        let (fetch_extra, refill) = if self.pipeline_valid {
            self.regs[15] = self.regs[15].wrapping_add(4);
            let fe = if bus.rom_data_accessed || bus.write_wait_cycles > 0 {
                bus.code_fetch_extra(instr_addr, false, false)
            } else {
                0
            };
            (fe, 0)
        } else {
            let target = self.regs[15];
            let rf = bus.branch_refill(target, self.in_thumb());
            (0, rf)
        };

        let base = cycles + bus.data_wait_cycles + bus.write_wait_cycles + fetch_extra + refill;
        let total = base + stall;
        bus.prev_exec_cycles = base;
        bus.prev_was_branch = !self.pipeline_valid;

        #[cfg(feature = "native-test")]
        {
            bus.debug_stall_total += stall as u64;
            bus.debug_refill_total += refill as u64;
            bus.debug_instrs_frame += 1;
            bus.debug_cumulative_instrs += 1;
        }

        total
    }

    pub fn execute_thumb(&mut self, bus: &mut Bus) -> u32 {
        let instr_addr = self.regs[15].wrapping_sub(4);
        let stall = bus.pipeline_stall(instr_addr, true);
        bus.fetching_code = true;
        let instr = bus.read16(instr_addr) as u16;
        bus.fetching_code = false;
        bus.data_wait_cycles = 0;
        bus.write_wait_cycles = 0;
        bus.rom_data_accessed = false;
        bus.last_rom_data_addr = 0xFFFF_FFFF;

        self.pipeline_valid = true;
        let cycles = crate::thumb::execute(self, bus, instr);

        let (fetch_extra, refill) = if self.pipeline_valid {
            self.regs[15] = self.regs[15].wrapping_add(2);
            let fe = if bus.rom_data_accessed || bus.write_wait_cycles > 0 {
                bus.code_fetch_extra(instr_addr, true, false)
            } else {
                0
            };
            (fe, 0)
        } else {
            let target = self.regs[15];
            let rf = bus.branch_refill(target, self.in_thumb());
            (0, rf)
        };

        let base = cycles + bus.data_wait_cycles + bus.write_wait_cycles + fetch_extra + refill;
        let total = base + stall;
        bus.prev_exec_cycles = base;
        bus.prev_was_branch = !self.pipeline_valid;

        #[cfg(feature = "native-test")]
        {
            bus.debug_stall_total += stall as u64;
            bus.debug_refill_total += refill as u64;
            bus.debug_instrs_frame += 1;
            bus.debug_cumulative_instrs += 1;
            if false {
                eprintln!("  T @{:08X}: cyc={} dw={} fe={} rf={} st={} tot={} rda={}",
                    instr_addr, cycles, bus.data_wait_cycles, fetch_extra, refill, stall, total, bus.rom_data_accessed);
            }
        }

        total
    }

    pub fn set_nz(&mut self, val: u32) {
        self.cpsr &= !(N_FLAG | Z_FLAG);
        if val == 0 { self.cpsr |= Z_FLAG; }
        if val & 0x8000_0000 != 0 { self.cpsr |= N_FLAG; }
    }

    pub fn set_nzcv_add(&mut self, a: u32, b: u32, result: u32) {
        self.set_nz(result);
        self.cpsr &= !(C_FLAG | V_FLAG);
        if (result as u64) != (a as u64).wrapping_add(b as u64) {
            // Actually check carry properly
        }
        let carry = (a as u64) + (b as u64) > 0xFFFF_FFFF;
        let overflow = ((a ^ result) & (b ^ result)) >> 31 != 0;
        if carry { self.cpsr |= C_FLAG; }
        if overflow { self.cpsr |= V_FLAG; }
    }

    pub fn add_with_flags(&mut self, a: u32, b: u32, set_flags: bool) -> u32 {
        let result = a.wrapping_add(b);
        if set_flags {
            self.set_nz(result);
            self.cpsr &= !(C_FLAG | V_FLAG);
            if (a as u64) + (b as u64) > 0xFFFF_FFFF { self.cpsr |= C_FLAG; }
            if ((a ^ result) & (b ^ result)) >> 31 != 0 { self.cpsr |= V_FLAG; }
        }
        result
    }

    pub fn sub_with_flags(&mut self, a: u32, b: u32, set_flags: bool) -> u32 {
        let result = a.wrapping_sub(b);
        if set_flags {
            self.set_nz(result);
            self.cpsr &= !(C_FLAG | V_FLAG);
            if a >= b { self.cpsr |= C_FLAG; }
            if ((a ^ b) & (a ^ result)) >> 31 != 0 { self.cpsr |= V_FLAG; }
        }
        result
    }

    pub fn adc_with_flags(&mut self, a: u32, b: u32, set_flags: bool) -> u32 {
        let c = if self.cpsr & C_FLAG != 0 { 1u64 } else { 0u64 };
        let result64 = (a as u64) + (b as u64) + c;
        let result = result64 as u32;
        if set_flags {
            self.set_nz(result);
            self.cpsr &= !(C_FLAG | V_FLAG);
            if result64 > 0xFFFF_FFFF { self.cpsr |= C_FLAG; }
            if ((a ^ result) & (b ^ result)) >> 31 != 0 { self.cpsr |= V_FLAG; }
        }
        result
    }

    pub fn sbc_with_flags(&mut self, a: u32, b: u32, set_flags: bool) -> u32 {
        let c = if self.cpsr & C_FLAG != 0 { 1u32 } else { 0u32 };
        let result = a.wrapping_sub(b).wrapping_sub(1).wrapping_add(c);
        if set_flags {
            self.set_nz(result);
            self.cpsr &= !(C_FLAG | V_FLAG);
            let borrow = (a as u64) >= (b as u64) + (1 - c as u64);
            if borrow { self.cpsr |= C_FLAG; }
            if ((a ^ b) & (a ^ result)) >> 31 != 0 { self.cpsr |= V_FLAG; }
        }
        result
    }

    pub fn barrel_shift(&mut self, val: u32, shift_type: u32, amount: u32, set_carry: bool, reg_shift: bool) -> u32 {
        if amount == 0 && !reg_shift {
            match shift_type {
                0 => val,
                1 => {
                    if set_carry {
                        self.cpsr = (self.cpsr & !C_FLAG) | if val >> 31 != 0 { C_FLAG } else { 0 };
                    }
                    0
                }
                2 => {
                    let bit = val >> 31;
                    if set_carry {
                        self.cpsr = (self.cpsr & !C_FLAG) | if bit != 0 { C_FLAG } else { 0 };
                    }
                    if bit != 0 { 0xFFFF_FFFF } else { 0 }
                }
                3 => {
                    let c = if self.cpsr & C_FLAG != 0 { 1u32 } else { 0 };
                    let result = (c << 31) | (val >> 1);
                    if set_carry {
                        self.cpsr = (self.cpsr & !C_FLAG) | if val & 1 != 0 { C_FLAG } else { 0 };
                    }
                    result
                }
                _ => val,
            }
        } else if amount == 0 && reg_shift {
            val
        } else {
            match shift_type {
                0 => {
                    if amount >= 32 {
                        if set_carry {
                            let c = if amount == 32 { val & 1 } else { 0 };
                            self.cpsr = (self.cpsr & !C_FLAG) | if c != 0 { C_FLAG } else { 0 };
                        }
                        0
                    } else {
                        if set_carry {
                            let c = (val >> (32 - amount)) & 1;
                            self.cpsr = (self.cpsr & !C_FLAG) | if c != 0 { C_FLAG } else { 0 };
                        }
                        val << amount
                    }
                }
                1 => {
                    if amount >= 32 {
                        if set_carry {
                            let c = if amount == 32 { val >> 31 } else { 0 };
                            self.cpsr = (self.cpsr & !C_FLAG) | if c != 0 { C_FLAG } else { 0 };
                        }
                        0
                    } else {
                        if set_carry {
                            let c = (val >> (amount - 1)) & 1;
                            self.cpsr = (self.cpsr & !C_FLAG) | if c != 0 { C_FLAG } else { 0 };
                        }
                        val >> amount
                    }
                }
                2 => {
                    if amount >= 32 {
                        let bit = val >> 31;
                        if set_carry {
                            self.cpsr = (self.cpsr & !C_FLAG) | if bit != 0 { C_FLAG } else { 0 };
                        }
                        if bit != 0 { 0xFFFF_FFFF } else { 0 }
                    } else {
                        if set_carry {
                            let c = ((val as i32) >> (amount - 1)) as u32 & 1;
                            self.cpsr = (self.cpsr & !C_FLAG) | if c != 0 { C_FLAG } else { 0 };
                        }
                        ((val as i32) >> amount) as u32
                    }
                }
                3 => {
                    let amount = amount & 31;
                    if amount == 0 {
                        if set_carry {
                            self.cpsr = (self.cpsr & !C_FLAG) | if val >> 31 != 0 { C_FLAG } else { 0 };
                        }
                        val
                    } else {
                        if set_carry {
                            let c = (val >> (amount - 1)) & 1;
                            self.cpsr = (self.cpsr & !C_FLAG) | if c != 0 { C_FLAG } else { 0 };
                        }
                        val.rotate_right(amount)
                    }
                }
                _ => val,
            }
        }
    }

    pub fn software_interrupt(&mut self, _comment: u32, _bus: &Bus) {
        #[cfg(feature = "native-test")]
        {
            eprintln!("  SWI 0x{:02X} from PC=0x{:08X} at cycle={} scanline={} frame={}",
                _comment,
                self.regs[15].wrapping_sub(if self.cpsr & T_FLAG != 0 { 4 } else { 8 }),
                _bus.total_cycles, _bus.current_scanline, _bus.frame_count);
            if _comment == 0x0B || _comment == 0x0C {
                eprintln!("    r0=0x{:08X} r1=0x{:08X} r2=0x{:08X}",
                    self.regs[0], self.regs[1], self.regs[2]);
            }
        }
        let old_cpsr = self.cpsr;
        self.switch_mode(MODE_SVC);
        self.spsr_svc = old_cpsr;
        self.regs[14] = if old_cpsr & T_FLAG != 0 {
            self.regs[15].wrapping_sub(2)
        } else {
            self.regs[15].wrapping_sub(4)
        };
        self.cpsr &= !T_FLAG;
        self.cpsr |= I_FLAG;
        self.regs[15] = 0x08;
        self.pipeline_valid = false;
    }
}
