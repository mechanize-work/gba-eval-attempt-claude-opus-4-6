use crate::bus::Bus;
use crate::cpu::*;

pub fn execute(cpu: &mut Cpu, bus: &mut Bus, instr: u32) -> u32 {
    let bits_27_25 = (instr >> 25) & 0x7;
    let bit4 = (instr >> 4) & 1;
    let bit7 = (instr >> 7) & 1;

    match bits_27_25 {
        0b000 => {
            if instr & 0x0FFF_FFF0 == 0x012F_FF10 {
                return arm_bx(cpu, bus, instr);
            }
            if (instr & 0x0FB0_0FF0) == 0x0100_0090 {
                return arm_swp(cpu, bus, instr);
            }
            if (instr & 0x0FC0_00F0) == 0x0000_0090 {
                return arm_multiply(cpu, bus, instr);
            }
            if (instr & 0x0F80_00F0) == 0x0080_0090 {
                return arm_multiply_long(cpu, bus, instr);
            }
            if bit7 == 1 && bit4 == 1 {
                let op = (instr >> 5) & 3;
                if op != 0 {
                    return arm_halfword_transfer(cpu, bus, instr);
                }
                return arm_halfword_transfer(cpu, bus, instr);
            }
            arm_data_processing(cpu, bus, instr)
        }
        0b001 => {
            let opcode = (instr >> 21) & 0xF;
            if (opcode & 0xC) == 0x8 && (instr >> 20) & 1 == 0 {
                return arm_msr(cpu, bus, instr);
            }
            arm_data_processing(cpu, bus, instr)
        }
        0b010 => arm_single_transfer(cpu, bus, instr),
        0b011 => {
            if bit4 == 1 {
                1
            } else {
                arm_single_transfer(cpu, bus, instr)
            }
        }
        0b100 => arm_block_transfer(cpu, bus, instr),
        0b101 => arm_branch(cpu, bus, instr),
        0b110 => 1,
        0b111 => {
            if (instr >> 24) & 1 == 1 {
                arm_swi(cpu, bus, instr)
            } else {
                1
            }
        }
        _ => 1,
    }
}

fn arm_data_processing(cpu: &mut Cpu, _bus: &mut Bus, instr: u32) -> u32 {
    let imm = (instr >> 25) & 1 != 0;
    let opcode = (instr >> 21) & 0xF;
    let set_flags = (instr >> 20) & 1 != 0;
    let rn = ((instr >> 16) & 0xF) as usize;
    let rd = ((instr >> 12) & 0xF) as usize;

    if !set_flags && (opcode >= 0x8 && opcode <= 0xB) {
        return arm_psr_transfer(cpu, instr);
    }

    let rn_val = cpu.regs[rn];

    let (op2, shift_carry) = if imm {
        let imm_val = instr & 0xFF;
        let rotate = ((instr >> 8) & 0xF) * 2;
        let result = imm_val.rotate_right(rotate);
        let carry = if rotate == 0 {
            cpu.cpsr & C_FLAG != 0
        } else {
            result >> 31 != 0
        };
        (result, carry)
    } else {
        let rm = (instr & 0xF) as usize;
        let shift_type = (instr >> 5) & 3;
        let reg_shift = (instr >> 4) & 1 != 0;

        let shift_amount = if reg_shift {
            let rs = ((instr >> 8) & 0xF) as usize;
            cpu.regs[rs] & 0xFF
        } else {
            (instr >> 7) & 0x1F
        };

        let rm_val = if reg_shift && rm == 15 {
            cpu.regs[15]
        } else {
            cpu.regs[rm]
        };

        let old_carry = cpu.cpsr & C_FLAG != 0;
        let result = cpu.barrel_shift(rm_val, shift_type, shift_amount, true, reg_shift);
        let new_carry = cpu.cpsr & C_FLAG != 0;

        if !set_flags {
            if old_carry { cpu.cpsr |= C_FLAG; } else { cpu.cpsr &= !C_FLAG; }
        }

        (result, new_carry)
    };

    let result = match opcode {
        0x0 => { // AND
            let r = rn_val & op2;
            if set_flags {
                cpu.set_nz(r);
                cpu.cpsr = (cpu.cpsr & !C_FLAG) | if shift_carry { C_FLAG } else { 0 };
            }
            if rd != 15 { cpu.regs[rd] = r; }
            r
        }
        0x1 => { // EOR
            let r = rn_val ^ op2;
            if set_flags {
                cpu.set_nz(r);
                cpu.cpsr = (cpu.cpsr & !C_FLAG) | if shift_carry { C_FLAG } else { 0 };
            }
            if rd != 15 { cpu.regs[rd] = r; }
            r
        }
        0x2 => { // SUB
            let r = cpu.sub_with_flags(rn_val, op2, set_flags);
            if rd != 15 { cpu.regs[rd] = r; }
            r
        }
        0x3 => { // RSB
            let r = cpu.sub_with_flags(op2, rn_val, set_flags);
            if rd != 15 { cpu.regs[rd] = r; }
            r
        }
        0x4 => { // ADD
            let r = cpu.add_with_flags(rn_val, op2, set_flags);
            if rd != 15 { cpu.regs[rd] = r; }
            r
        }
        0x5 => { // ADC
            let r = cpu.adc_with_flags(rn_val, op2, set_flags);
            if rd != 15 { cpu.regs[rd] = r; }
            r
        }
        0x6 => { // SBC
            let r = cpu.sbc_with_flags(rn_val, op2, set_flags);
            if rd != 15 { cpu.regs[rd] = r; }
            r
        }
        0x7 => { // RSC
            let r = cpu.sbc_with_flags(op2, rn_val, set_flags);
            if rd != 15 { cpu.regs[rd] = r; }
            r
        }
        0x8 => { // TST
            let r = rn_val & op2;
            cpu.set_nz(r);
            cpu.cpsr = (cpu.cpsr & !C_FLAG) | if shift_carry { C_FLAG } else { 0 };
            r
        }
        0x9 => { // TEQ
            let r = rn_val ^ op2;
            cpu.set_nz(r);
            cpu.cpsr = (cpu.cpsr & !C_FLAG) | if shift_carry { C_FLAG } else { 0 };
            r
        }
        0xA => { // CMP
            cpu.sub_with_flags(rn_val, op2, true);
            0
        }
        0xB => { // CMN
            cpu.add_with_flags(rn_val, op2, true);
            0
        }
        0xC => { // ORR
            let r = rn_val | op2;
            if set_flags {
                cpu.set_nz(r);
                cpu.cpsr = (cpu.cpsr & !C_FLAG) | if shift_carry { C_FLAG } else { 0 };
            }
            if rd != 15 { cpu.regs[rd] = r; }
            r
        }
        0xD => { // MOV
            if set_flags {
                cpu.set_nz(op2);
                cpu.cpsr = (cpu.cpsr & !C_FLAG) | if shift_carry { C_FLAG } else { 0 };
            }
            if rd != 15 { cpu.regs[rd] = op2; }
            op2
        }
        0xE => { // BIC
            let r = rn_val & !op2;
            if set_flags {
                cpu.set_nz(r);
                cpu.cpsr = (cpu.cpsr & !C_FLAG) | if shift_carry { C_FLAG } else { 0 };
            }
            if rd != 15 { cpu.regs[rd] = r; }
            r
        }
        0xF => { // MVN
            let r = !op2;
            if set_flags {
                cpu.set_nz(r);
                cpu.cpsr = (cpu.cpsr & !C_FLAG) | if shift_carry { C_FLAG } else { 0 };
            }
            if rd != 15 { cpu.regs[rd] = r; }
            r
        }
        _ => 0,
    };

    if rd == 15 {
        if set_flags {
            let spsr = cpu.get_spsr();
            cpu.switch_mode(spsr & 0x1F);
            cpu.cpsr = spsr;
        }
        cpu.regs[15] = result & if cpu.in_thumb() { !1 } else { !3 };
        cpu.pipeline_valid = false;
        return 3;
    }

    if !imm && (instr >> 4) & 1 != 0 {
        2
    } else {
        1
    }
}

fn arm_psr_transfer(cpu: &mut Cpu, instr: u32) -> u32 {
    let is_spsr = (instr >> 22) & 1 != 0;
    let is_msr = (instr >> 21) & 1 != 0;

    if !is_msr {
        let rd = ((instr >> 12) & 0xF) as usize;
        let val = if is_spsr { cpu.get_spsr() } else { cpu.cpsr };
        cpu.regs[rd] = val;
        1
    } else {
        let val = if (instr >> 25) & 1 != 0 {
            let imm = instr & 0xFF;
            let rotate = ((instr >> 8) & 0xF) * 2;
            imm.rotate_right(rotate)
        } else {
            let rm = (instr & 0xF) as usize;
            cpu.regs[rm]
        };

        let mut mask = 0u32;
        if instr & (1 << 19) != 0 { mask |= 0xFF00_0000; }
        if instr & (1 << 18) != 0 { mask |= 0x00FF_0000; }
        if instr & (1 << 17) != 0 { mask |= 0x0000_FF00; }
        if instr & (1 << 16) != 0 { mask |= 0x0000_00FF; }

        let mode = cpu.cpsr & 0x1F;
        if mode == MODE_USR {
            mask &= 0xFF00_0000;
        }

        if is_spsr {
            let spsr = cpu.get_spsr();
            cpu.set_spsr((spsr & !mask) | (val & mask));
        } else {
            let new_cpsr = (cpu.cpsr & !mask) | (val & mask);
            if mask & 0x1F != 0 {
                let new_mode = new_cpsr & 0x1F;
                cpu.switch_mode(new_mode);
            }
            cpu.cpsr = (cpu.cpsr & !mask) | (val & mask);
        }
        1
    }
}

fn arm_msr(cpu: &mut Cpu, _bus: &mut Bus, instr: u32) -> u32 {
    arm_psr_transfer(cpu, instr)
}

fn arm_branch(cpu: &mut Cpu, _bus: &mut Bus, instr: u32) -> u32 {
    let link = (instr >> 24) & 1 != 0;
    let offset = ((instr & 0x00FF_FFFF) << 2) as i32;
    let offset = (offset << 6) >> 6;

    if link {
        cpu.regs[14] = cpu.regs[15].wrapping_sub(4);
    }

    cpu.regs[15] = (cpu.regs[15] as i32).wrapping_add(offset) as u32;
    cpu.pipeline_valid = false;
    3
}

fn arm_bx(cpu: &mut Cpu, _bus: &mut Bus, instr: u32) -> u32 {
    let rm = (instr & 0xF) as usize;
    let addr = cpu.regs[rm];

    if addr & 1 != 0 {
        cpu.cpsr |= T_FLAG;
        cpu.regs[15] = addr & !1;
    } else {
        cpu.cpsr &= !T_FLAG;
        cpu.regs[15] = addr & !3;
    }
    cpu.pipeline_valid = false;
    3
}

fn arm_single_transfer(cpu: &mut Cpu, bus: &mut Bus, instr: u32) -> u32 {
    let imm = (instr >> 25) & 1 == 0;
    let pre = (instr >> 24) & 1 != 0;
    let up = (instr >> 23) & 1 != 0;
    let byte = (instr >> 22) & 1 != 0;
    let writeback = (instr >> 21) & 1 != 0;
    let load = (instr >> 20) & 1 != 0;
    let rn = ((instr >> 16) & 0xF) as usize;
    let rd = ((instr >> 12) & 0xF) as usize;

    let offset = if imm {
        instr & 0xFFF
    } else {
        let rm = (instr & 0xF) as usize;
        let shift_type = (instr >> 5) & 3;
        let shift_amount = (instr >> 7) & 0x1F;
        let rm_val = cpu.regs[rm];
        cpu.barrel_shift(rm_val, shift_type, shift_amount, false, false)
    };

    let base = cpu.regs[rn];
    let addr = if pre {
        if up { base.wrapping_add(offset) } else { base.wrapping_sub(offset) }
    } else {
        base
    };

    let mut cycles = 1;

    if load {
        let val = if byte {
            bus.read8(addr) as u32
        } else {
            let val = bus.read32(addr & !3);
            let rot = (addr & 3) * 8;
            val.rotate_right(rot)
        };
        if rd == 15 {
            cpu.regs[15] = val & !3;
            cpu.pipeline_valid = false;
            cycles = 5;
        } else {
            cpu.regs[rd] = val;
            cycles = 3;
        }
    } else {
        let val = if rd == 15 {
            cpu.regs[15]
        } else {
            cpu.regs[rd]
        };
        if byte {
            bus.write8(addr, val as u8);
        } else {
            bus.write32(addr & !3, val);
        }
        cycles = 2;
    }

    let wb_addr = if !pre {
        if up { base.wrapping_add(offset) } else { base.wrapping_sub(offset) }
    } else {
        addr
    };

    if (!pre || writeback) && (!load || rd != rn) {
        cpu.regs[rn] = wb_addr;
    }

    cycles
}

fn arm_halfword_transfer(cpu: &mut Cpu, bus: &mut Bus, instr: u32) -> u32 {
    let pre = (instr >> 24) & 1 != 0;
    let up = (instr >> 23) & 1 != 0;
    let imm_offset = (instr >> 22) & 1 != 0;
    let writeback = (instr >> 21) & 1 != 0;
    let load = (instr >> 20) & 1 != 0;
    let rn = ((instr >> 16) & 0xF) as usize;
    let rd = ((instr >> 12) & 0xF) as usize;
    let op = (instr >> 5) & 3;

    let offset = if imm_offset {
        ((instr >> 4) & 0xF0) | (instr & 0xF)
    } else {
        let rm = (instr & 0xF) as usize;
        cpu.regs[rm]
    };

    let base = cpu.regs[rn];
    let addr = if pre {
        if up { base.wrapping_add(offset) } else { base.wrapping_sub(offset) }
    } else {
        base
    };

    let mut cycles = 1;

    if load {
        let val = match op {
            1 => { // LDRH
                let v = bus.read16(addr & !1) as u32;
                v
            }
            2 => { // LDRSB
                let v = bus.read8(addr) as i8 as i32 as u32;
                v
            }
            3 => { // LDRSH
                let v = if addr & 1 != 0 {
                    bus.read8(addr) as i8 as i32 as u32
                } else {
                    bus.read16(addr) as i16 as i32 as u32
                };
                v
            }
            _ => 0,
        };
        cpu.regs[rd] = val;
        cycles = 3;
        if rd == 15 {
            cpu.pipeline_valid = false;
            cycles = 5;
        }
    } else {
        match op {
            1 => { // STRH
                let val = cpu.regs[rd] as u16;
                bus.write16(addr & !1, val);
            }
            _ => {}
        }
        cycles = 2;
    }

    let wb_addr = if !pre {
        if up { base.wrapping_add(offset) } else { base.wrapping_sub(offset) }
    } else {
        addr
    };

    if (!pre || writeback) && (!load || rd != rn) {
        cpu.regs[rn] = wb_addr;
    }

    cycles
}

fn arm_block_transfer(cpu: &mut Cpu, bus: &mut Bus, instr: u32) -> u32 {
    let pre = (instr >> 24) & 1 != 0;
    let up = (instr >> 23) & 1 != 0;
    let s_bit = (instr >> 22) & 1 != 0;
    let writeback = (instr >> 21) & 1 != 0;
    let load = (instr >> 20) & 1 != 0;
    let rn = ((instr >> 16) & 0xF) as usize;
    let rlist = instr & 0xFFFF;

    let reg_count = rlist.count_ones();
    let base = cpu.regs[rn];

    if rlist == 0 {
        if load {
            cpu.regs[15] = bus.read32(base);
            cpu.pipeline_valid = false;
        } else {
            bus.write32(base, cpu.regs[15]);
        }
        if writeback {
            cpu.regs[rn] = if up { base.wrapping_add(0x40) } else { base.wrapping_sub(0x40) };
        }
        return 3;
    }

    let wb_val = if up {
        base.wrapping_add(reg_count * 4)
    } else {
        base.wrapping_sub(reg_count * 4)
    };

    let mut addr = match (pre, up) {
        (false, true) => base,
        (true, true) => base.wrapping_add(4),
        (false, false) => base.wrapping_sub(reg_count * 4).wrapping_add(4),
        (true, false) => base.wrapping_sub(reg_count * 4),
    };

    let user_mode = s_bit && (!load || rlist & (1 << 15) == 0);
    let old_mode = cpu.cpsr & 0x1F;
    if user_mode {
        cpu.switch_mode(MODE_USR);
    }

    let mut first = true;
    let mut cycles = 0u32;

    if load {
        for i in 0..16 {
            if rlist & (1 << i) != 0 {
                let val = bus.read32(addr & !3);
                cpu.regs[i] = val;
                addr = addr.wrapping_add(4);
                if first {
                    first = false;
                    if writeback && !user_mode {
                        cpu.regs[rn] = wb_val;
                    }
                }
                cycles += 1;
            }
        }
        cycles += 2;

        if rlist & (1 << 15) != 0 {
            if s_bit {
                let spsr = cpu.get_spsr();
                if user_mode { cpu.switch_mode(old_mode); }
                cpu.switch_mode(spsr & 0x1F);
                cpu.cpsr = spsr;
            }
            cpu.regs[15] &= if cpu.in_thumb() { !1 } else { !3 };
            cpu.pipeline_valid = false;
        }
    } else {
        for i in 0..16 {
            if rlist & (1 << i) != 0 {
                let val = if i == 15 {
                    cpu.regs[15]
                } else {
                    cpu.regs[i]
                };
                bus.write32(addr & !3, val);
                addr = addr.wrapping_add(4);
                if first {
                    first = false;
                    if writeback && !user_mode {
                        cpu.regs[rn] = wb_val;
                    }
                }
                cycles += 1;
            }
        }
        cycles += 1;
    }

    if user_mode {
        cpu.switch_mode(old_mode);
    } else if writeback {
        cpu.regs[rn] = wb_val;
    }

    cycles
}

fn arm_multiply(cpu: &mut Cpu, _bus: &mut Bus, instr: u32) -> u32 {
    let acc = (instr >> 21) & 1 != 0;
    let set_flags = (instr >> 20) & 1 != 0;
    let rd = ((instr >> 16) & 0xF) as usize;
    let rn = ((instr >> 12) & 0xF) as usize;
    let rs = ((instr >> 8) & 0xF) as usize;
    let rm = (instr & 0xF) as usize;

    let mut result = cpu.regs[rm].wrapping_mul(cpu.regs[rs]);
    if acc {
        result = result.wrapping_add(cpu.regs[rn]);
    }
    cpu.regs[rd] = result;

    if set_flags {
        cpu.set_nz(result);
    }

    let m = multiply_cycles(cpu.regs[rs]);
    m + if acc { 1 } else { 0 }
}

fn arm_multiply_long(cpu: &mut Cpu, _bus: &mut Bus, instr: u32) -> u32 {
    let sign = (instr >> 22) & 1 != 0;
    let acc = (instr >> 21) & 1 != 0;
    let set_flags = (instr >> 20) & 1 != 0;
    let rdhi = ((instr >> 16) & 0xF) as usize;
    let rdlo = ((instr >> 12) & 0xF) as usize;
    let rs = ((instr >> 8) & 0xF) as usize;
    let rm = (instr & 0xF) as usize;

    let result = if sign {
        let a = cpu.regs[rm] as i32 as i64;
        let b = cpu.regs[rs] as i32 as i64;
        let mut r = a.wrapping_mul(b) as u64;
        if acc {
            let old = ((cpu.regs[rdhi] as u64) << 32) | (cpu.regs[rdlo] as u64);
            r = r.wrapping_add(old);
        }
        r
    } else {
        let a = cpu.regs[rm] as u64;
        let b = cpu.regs[rs] as u64;
        let mut r = a.wrapping_mul(b);
        if acc {
            let old = ((cpu.regs[rdhi] as u64) << 32) | (cpu.regs[rdlo] as u64);
            r = r.wrapping_add(old);
        }
        r
    };

    cpu.regs[rdhi] = (result >> 32) as u32;
    cpu.regs[rdlo] = result as u32;

    if set_flags {
        cpu.cpsr &= !(N_FLAG | Z_FLAG);
        if result == 0 { cpu.cpsr |= Z_FLAG; }
        if result >> 63 != 0 { cpu.cpsr |= N_FLAG; }
    }

    let m = multiply_cycles(cpu.regs[rs]);
    m + 1 + if acc { 1 } else { 0 }
}

fn arm_swp(cpu: &mut Cpu, bus: &mut Bus, instr: u32) -> u32 {
    let byte = (instr >> 22) & 1 != 0;
    let rn = ((instr >> 16) & 0xF) as usize;
    let rd = ((instr >> 12) & 0xF) as usize;
    let rm = (instr & 0xF) as usize;

    let addr = cpu.regs[rn];

    if byte {
        let old = bus.read8(addr) as u32;
        bus.write8(addr, cpu.regs[rm] as u8);
        cpu.regs[rd] = old;
    } else {
        let old = bus.read32(addr & !3);
        let rot = (addr & 3) * 8;
        let old = old.rotate_right(rot);
        bus.write32(addr & !3, cpu.regs[rm]);
        cpu.regs[rd] = old;
    }

    4
}

fn arm_swi(cpu: &mut Cpu, _bus: &mut Bus, instr: u32) -> u32 {
    let comment = (instr >> 16) & 0xFF;
    cpu.software_interrupt(comment, _bus);
    3
}

fn multiply_cycles(rs: u32) -> u32 {
    if rs & 0xFFFF_FF00 == 0 || rs & 0xFFFF_FF00 == 0xFFFF_FF00 { 1 }
    else if rs & 0xFFFF_0000 == 0 || rs & 0xFFFF_0000 == 0xFFFF_0000 { 2 }
    else if rs & 0xFF00_0000 == 0 || rs & 0xFF00_0000 == 0xFF00_0000 { 3 }
    else { 4 }
}
