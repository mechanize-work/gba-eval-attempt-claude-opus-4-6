use crate::bus::Bus;
use crate::cpu::*;

pub fn execute(cpu: &mut Cpu, bus: &mut Bus, instr: u16) -> u32 {
    let top = (instr >> 8) as u8;
    match top >> 3 {
        0b00000..=0b00010 => thumb_shift(cpu, instr),
        0b00011 => thumb_add_sub(cpu, instr),
        0b00100..=0b00111 => thumb_mov_cmp_add_sub_imm(cpu, instr),
        0b01000 => {
            if (instr >> 10) & 3 == 0 {
                thumb_alu(cpu, bus, instr)
            } else if (instr >> 10) & 3 == 1 {
                thumb_hi_reg(cpu, bus, instr)
            } else {
                thumb_pc_rel_load(cpu, bus, instr)
            }
        }
        0b01001 => thumb_pc_rel_load(cpu, bus, instr),
        0b01010 | 0b01011 => {
            if (instr >> 9) & 1 == 0 {
                thumb_load_store_reg(cpu, bus, instr)
            } else {
                thumb_load_store_sign(cpu, bus, instr)
            }
        }
        0b01100..=0b01111 => thumb_load_store_imm(cpu, bus, instr),
        0b10000 | 0b10001 => thumb_load_store_half(cpu, bus, instr),
        0b10010 | 0b10011 => thumb_sp_rel_load_store(cpu, bus, instr),
        0b10100 | 0b10101 => thumb_load_addr(cpu, instr),
        0b10110 | 0b10111 => {
            if (instr >> 12) & 0xF == 0xB {
                match (instr >> 8) & 0xF {
                    0x0 => thumb_adjust_sp(cpu, instr),
                    0x4 | 0x5 | 0xC | 0xD => thumb_push_pop(cpu, bus, instr),
                    _ => 1,
                }
            } else {
                1
            }
        }
        0b11000 | 0b11001 => thumb_multiple_load_store(cpu, bus, instr),
        0b11010 | 0b11011 => {
            if (instr >> 8) & 0xF == 0xF {
                thumb_swi(cpu, bus, instr)
            } else {
                thumb_cond_branch(cpu, instr)
            }
        }
        0b11100 => thumb_uncond_branch(cpu, instr),
        0b11110 | 0b11111 => thumb_long_branch(cpu, instr),
        _ => 1,
    }
}

fn thumb_shift(cpu: &mut Cpu, instr: u16) -> u32 {
    let op = (instr >> 11) & 3;
    let offset = ((instr >> 6) & 0x1F) as u32;
    let rs = ((instr >> 3) & 7) as usize;
    let rd = (instr & 7) as usize;
    let val = cpu.regs[rs];

    let result = match op {
        0 => { // LSL
            if offset == 0 {
                val
            } else {
                let c = (val >> (32 - offset)) & 1;
                cpu.cpsr = (cpu.cpsr & !C_FLAG) | if c != 0 { C_FLAG } else { 0 };
                val << offset
            }
        }
        1 => { // LSR
            let shift = if offset == 0 { 32 } else { offset };
            if shift == 32 {
                cpu.cpsr = (cpu.cpsr & !C_FLAG) | if val >> 31 != 0 { C_FLAG } else { 0 };
                0
            } else {
                cpu.cpsr = (cpu.cpsr & !C_FLAG) | if (val >> (shift - 1)) & 1 != 0 { C_FLAG } else { 0 };
                val >> shift
            }
        }
        2 => { // ASR
            let shift = if offset == 0 { 32 } else { offset };
            if shift >= 32 {
                let bit = val >> 31;
                cpu.cpsr = (cpu.cpsr & !C_FLAG) | if bit != 0 { C_FLAG } else { 0 };
                if bit != 0 { 0xFFFF_FFFF } else { 0 }
            } else {
                cpu.cpsr = (cpu.cpsr & !C_FLAG) | if ((val as i32) >> (shift - 1)) & 1 != 0 { C_FLAG } else { 0 };
                ((val as i32) >> shift) as u32
            }
        }
        _ => val,
    };

    cpu.regs[rd] = result;
    cpu.set_nz(result);
    1
}

fn thumb_add_sub(cpu: &mut Cpu, instr: u16) -> u32 {
    let imm = (instr >> 10) & 1 != 0;
    let sub = (instr >> 9) & 1 != 0;
    let rn_or_imm = ((instr >> 6) & 7) as u32;
    let rs = ((instr >> 3) & 7) as usize;
    let rd = (instr & 7) as usize;

    let op1 = cpu.regs[rs];
    let op2 = if imm { rn_or_imm } else { cpu.regs[rn_or_imm as usize] };

    let result = if sub {
        cpu.sub_with_flags(op1, op2, true)
    } else {
        cpu.add_with_flags(op1, op2, true)
    };

    cpu.regs[rd] = result;
    1
}

fn thumb_mov_cmp_add_sub_imm(cpu: &mut Cpu, instr: u16) -> u32 {
    let op = (instr >> 11) & 3;
    let rd = ((instr >> 8) & 7) as usize;
    let imm = (instr & 0xFF) as u32;

    match op {
        0 => { // MOV
            cpu.regs[rd] = imm;
            cpu.set_nz(imm);
        }
        1 => { // CMP
            cpu.sub_with_flags(cpu.regs[rd], imm, true);
        }
        2 => { // ADD
            let r = cpu.add_with_flags(cpu.regs[rd], imm, true);
            cpu.regs[rd] = r;
        }
        3 => { // SUB
            let r = cpu.sub_with_flags(cpu.regs[rd], imm, true);
            cpu.regs[rd] = r;
        }
        _ => {}
    }
    1
}

fn thumb_alu(cpu: &mut Cpu, _bus: &mut Bus, instr: u16) -> u32 {
    let op = (instr >> 6) & 0xF;
    let rs = ((instr >> 3) & 7) as usize;
    let rd = (instr & 7) as usize;
    let a = cpu.regs[rd];
    let b = cpu.regs[rs];

    let mut cycles = 1u32;

    match op {
        0x0 => { // AND
            let r = a & b;
            cpu.regs[rd] = r;
            cpu.set_nz(r);
        }
        0x1 => { // EOR
            let r = a ^ b;
            cpu.regs[rd] = r;
            cpu.set_nz(r);
        }
        0x2 => { // LSL
            let shift = b & 0xFF;
            let r = cpu.barrel_shift(a, 0, shift, true, true);
            cpu.regs[rd] = r;
            cpu.set_nz(r);
            cycles = 2;
        }
        0x3 => { // LSR
            let shift = b & 0xFF;
            let r = cpu.barrel_shift(a, 1, shift, true, true);
            cpu.regs[rd] = r;
            cpu.set_nz(r);
            cycles = 2;
        }
        0x4 => { // ASR
            let shift = b & 0xFF;
            let r = cpu.barrel_shift(a, 2, shift, true, true);
            cpu.regs[rd] = r;
            cpu.set_nz(r);
            cycles = 2;
        }
        0x5 => { // ADC
            let r = cpu.adc_with_flags(a, b, true);
            cpu.regs[rd] = r;
        }
        0x6 => { // SBC
            let r = cpu.sbc_with_flags(a, b, true);
            cpu.regs[rd] = r;
        }
        0x7 => { // ROR
            let shift = b & 0xFF;
            let r = cpu.barrel_shift(a, 3, shift, true, true);
            cpu.regs[rd] = r;
            cpu.set_nz(r);
            cycles = 2;
        }
        0x8 => { // TST
            let r = a & b;
            cpu.set_nz(r);
        }
        0x9 => { // NEG
            let r = cpu.sub_with_flags(0, b, true);
            cpu.regs[rd] = r;
        }
        0xA => { // CMP
            cpu.sub_with_flags(a, b, true);
        }
        0xB => { // CMN
            cpu.add_with_flags(a, b, true);
        }
        0xC => { // ORR
            let r = a | b;
            cpu.regs[rd] = r;
            cpu.set_nz(r);
        }
        0xD => { // MUL
            let r = a.wrapping_mul(b);
            cpu.regs[rd] = r;
            cpu.set_nz(r);
            cycles = 1 + multiply_cycles_thumb(a);
        }
        0xE => { // BIC
            let r = a & !b;
            cpu.regs[rd] = r;
            cpu.set_nz(r);
        }
        0xF => { // MVN
            let r = !b;
            cpu.regs[rd] = r;
            cpu.set_nz(r);
        }
        _ => {}
    }

    cycles
}

fn thumb_hi_reg(cpu: &mut Cpu, _bus: &mut Bus, instr: u16) -> u32 {
    let op = (instr >> 8) & 3;
    let h1 = (instr >> 7) & 1;
    let h2 = (instr >> 6) & 1;
    let rs = (((h2 << 3) | ((instr >> 3) & 7)) as usize) & 0xF;
    let rd = (((h1 << 3) | (instr & 7)) as usize) & 0xF;

    let a = cpu.regs[rd];
    let b = cpu.regs[rs];

    match op {
        0 => { // ADD
            let r = a.wrapping_add(b);
            cpu.regs[rd] = r;
            if rd == 15 {
                cpu.regs[15] &= !1;
                cpu.pipeline_valid = false;
                return 3;
            }
        }
        1 => { // CMP
            cpu.sub_with_flags(a, b, true);
        }
        2 => { // MOV
            cpu.regs[rd] = b;
            if rd == 15 {
                cpu.regs[15] &= !1;
                cpu.pipeline_valid = false;
                return 3;
            }
        }
        3 => { // BX
            if b & 1 != 0 {
                cpu.cpsr |= T_FLAG;
                cpu.regs[15] = b & !1;
            } else {
                cpu.cpsr &= !T_FLAG;
                cpu.regs[15] = b & !3;
            }
            cpu.pipeline_valid = false;
            return 3;
        }
        _ => {}
    }
    1
}

fn thumb_pc_rel_load(cpu: &mut Cpu, bus: &mut Bus, instr: u16) -> u32 {
    let rd = ((instr >> 8) & 7) as usize;
    let offset = ((instr & 0xFF) as u32) << 2;
    let addr = (cpu.regs[15] & !2).wrapping_add(offset);
    cpu.regs[rd] = bus.read32(addr & !3);
    3
}

fn thumb_load_store_reg(cpu: &mut Cpu, bus: &mut Bus, instr: u16) -> u32 {
    let op = (instr >> 10) & 3;
    let ro = ((instr >> 6) & 7) as usize;
    let rb = ((instr >> 3) & 7) as usize;
    let rd = (instr & 7) as usize;

    let addr = cpu.regs[rb].wrapping_add(cpu.regs[ro]);

    match op {
        0 => { // STR
            bus.write32(addr & !3, cpu.regs[rd]);
            2
        }
        1 => { // STRB
            bus.write8(addr, cpu.regs[rd] as u8);
            2
        }
        2 => { // LDR
            let val = bus.read32(addr & !3);
            let rot = (addr & 3) * 8;
            cpu.regs[rd] = val.rotate_right(rot);
            3
        }
        3 => { // LDRB
            cpu.regs[rd] = bus.read8(addr) as u32;
            3
        }
        _ => 1,
    }
}

fn thumb_load_store_sign(cpu: &mut Cpu, bus: &mut Bus, instr: u16) -> u32 {
    let op = (instr >> 10) & 3;
    let ro = ((instr >> 6) & 7) as usize;
    let rb = ((instr >> 3) & 7) as usize;
    let rd = (instr & 7) as usize;

    let addr = cpu.regs[rb].wrapping_add(cpu.regs[ro]);

    match op {
        0 => { // STRH
            bus.write16(addr & !1, cpu.regs[rd] as u16);
            2
        }
        1 => { // LDRSB
            cpu.regs[rd] = bus.read8(addr) as i8 as i32 as u32;
            3
        }
        2 => { // LDRH
            cpu.regs[rd] = bus.read16(addr & !1) as u32;
            3
        }
        3 => { // LDRSH
            if addr & 1 != 0 {
                cpu.regs[rd] = bus.read8(addr) as i8 as i32 as u32;
            } else {
                cpu.regs[rd] = bus.read16(addr) as i16 as i32 as u32;
            }
            3
        }
        _ => 1,
    }
}

fn thumb_load_store_imm(cpu: &mut Cpu, bus: &mut Bus, instr: u16) -> u32 {
    let byte = (instr >> 12) & 1 != 0;
    let load = (instr >> 11) & 1 != 0;
    let offset = ((instr >> 6) & 0x1F) as u32;
    let rb = ((instr >> 3) & 7) as usize;
    let rd = (instr & 7) as usize;

    let base = cpu.regs[rb];

    if byte {
        let addr = base.wrapping_add(offset);
        if load {
            cpu.regs[rd] = bus.read8(addr) as u32;
            3
        } else {
            bus.write8(addr, cpu.regs[rd] as u8);
            2
        }
    } else {
        let addr = base.wrapping_add(offset << 2);
        if load {
            let val = bus.read32(addr & !3);
            let rot = (addr & 3) * 8;
            cpu.regs[rd] = val.rotate_right(rot);
            3
        } else {
            bus.write32(addr & !3, cpu.regs[rd]);
            2
        }
    }
}

fn thumb_load_store_half(cpu: &mut Cpu, bus: &mut Bus, instr: u16) -> u32 {
    let load = (instr >> 11) & 1 != 0;
    let offset = (((instr >> 6) & 0x1F) as u32) << 1;
    let rb = ((instr >> 3) & 7) as usize;
    let rd = (instr & 7) as usize;

    let addr = cpu.regs[rb].wrapping_add(offset);

    if load {
        cpu.regs[rd] = bus.read16(addr & !1) as u32;
        3
    } else {
        bus.write16(addr & !1, cpu.regs[rd] as u16);
        2
    }
}

fn thumb_sp_rel_load_store(cpu: &mut Cpu, bus: &mut Bus, instr: u16) -> u32 {
    let load = (instr >> 11) & 1 != 0;
    let rd = ((instr >> 8) & 7) as usize;
    let offset = ((instr & 0xFF) as u32) << 2;
    let addr = cpu.regs[13].wrapping_add(offset);

    if load {
        cpu.regs[rd] = bus.read32(addr & !3);
        3
    } else {
        bus.write32(addr & !3, cpu.regs[rd]);
        2
    }
}

fn thumb_load_addr(cpu: &mut Cpu, instr: u16) -> u32 {
    let sp = (instr >> 11) & 1 != 0;
    let rd = ((instr >> 8) & 7) as usize;
    let offset = ((instr & 0xFF) as u32) << 2;

    if sp {
        cpu.regs[rd] = cpu.regs[13].wrapping_add(offset);
    } else {
        cpu.regs[rd] = (cpu.regs[15] & !2).wrapping_add(offset);
    }
    1
}

fn thumb_adjust_sp(cpu: &mut Cpu, instr: u16) -> u32 {
    let offset = ((instr & 0x7F) as u32) << 2;
    if (instr >> 7) & 1 != 0 {
        cpu.regs[13] = cpu.regs[13].wrapping_sub(offset);
    } else {
        cpu.regs[13] = cpu.regs[13].wrapping_add(offset);
    }
    1
}

fn thumb_push_pop(cpu: &mut Cpu, bus: &mut Bus, instr: u16) -> u32 {
    let pop = (instr >> 11) & 1 != 0;
    let pc_lr = (instr >> 8) & 1 != 0;
    let rlist = instr & 0xFF;

    let mut cycles = 0u32;

    if pop {
        let mut addr = cpu.regs[13];
        for i in 0..8 {
            if rlist & (1 << i) != 0 {
                cpu.regs[i] = bus.read32(addr & !3);
                addr = addr.wrapping_add(4);
                cycles += 1;
            }
        }
        if pc_lr {
            cpu.regs[15] = bus.read32(addr & !3) & !1;
            addr = addr.wrapping_add(4);
            cpu.pipeline_valid = false;
            cycles += 3;
        }
        cpu.regs[13] = addr;
        cycles += 2;
    } else {
        let reg_count = rlist.count_ones() + if pc_lr { 1 } else { 0 };
        let mut addr = cpu.regs[13].wrapping_sub(reg_count * 4);
        cpu.regs[13] = addr;
        for i in 0..8 {
            if rlist & (1 << i) != 0 {
                bus.write32(addr & !3, cpu.regs[i]);
                addr = addr.wrapping_add(4);
                cycles += 1;
            }
        }
        if pc_lr {
            bus.write32(addr & !3, cpu.regs[14]);
            cycles += 1;
        }
        cycles += 1;
    }

    cycles
}

fn thumb_multiple_load_store(cpu: &mut Cpu, bus: &mut Bus, instr: u16) -> u32 {
    let load = (instr >> 11) & 1 != 0;
    let rb = ((instr >> 8) & 7) as usize;
    let rlist = instr & 0xFF;

    let mut addr = cpu.regs[rb];
    let mut cycles = 0u32;

    if rlist == 0 {
        if load {
            cpu.regs[15] = bus.read32(addr);
            cpu.pipeline_valid = false;
        } else {
            bus.write32(addr, cpu.regs[15]);
        }
        cpu.regs[rb] = addr.wrapping_add(0x40);
        return 3;
    }

    if load {
        for i in 0..8 {
            if rlist & (1 << i) != 0 {
                cpu.regs[i] = bus.read32(addr & !3);
                addr = addr.wrapping_add(4);
                cycles += 1;
            }
        }
        if rlist & (1 << rb) == 0 {
            cpu.regs[rb] = addr;
        }
        cycles += 2;
    } else {
        let mut first = true;
        for i in 0..8 {
            if rlist & (1 << i) != 0 {
                bus.write32(addr & !3, cpu.regs[i]);
                addr = addr.wrapping_add(4);
                if first {
                    first = false;
                    cpu.regs[rb] = cpu.regs[rb].wrapping_add(rlist.count_ones() * 4);
                }
                cycles += 1;
            }
        }
        cycles += 1;
    }

    cycles
}

fn thumb_cond_branch(cpu: &mut Cpu, instr: u16) -> u32 {
    let cond = ((instr >> 8) & 0xF) as u32;
    if !cpu.check_condition(cond) {
        return 1;
    }
    let offset = (instr & 0xFF) as i8 as i32 as u32;
    cpu.regs[15] = cpu.regs[15].wrapping_add(offset << 1);
    cpu.pipeline_valid = false;
    3
}

fn thumb_swi(cpu: &mut Cpu, bus: &mut Bus, instr: u16) -> u32 {
    let comment = (instr & 0xFF) as u32;
    if bus.bios_hle(comment, cpu) {
        return 3;
    }
    cpu.software_interrupt(comment, bus);
    3
}

fn thumb_uncond_branch(cpu: &mut Cpu, instr: u16) -> u32 {
    let offset = ((instr & 0x7FF) as u32) << 1;
    let offset = if offset & 0x800 != 0 {
        offset | 0xFFFF_F000
    } else {
        offset
    };
    cpu.regs[15] = cpu.regs[15].wrapping_add(offset);
    cpu.pipeline_valid = false;
    3
}

fn thumb_long_branch(cpu: &mut Cpu, instr: u16) -> u32 {
    let h = (instr >> 11) & 1;
    let offset = (instr & 0x7FF) as u32;

    if h == 0 {
        let offset = offset << 12;
        let offset = if offset & 0x0040_0000 != 0 {
            offset | 0xFF80_0000
        } else {
            offset
        };
        cpu.regs[14] = cpu.regs[15].wrapping_add(offset);
        1
    } else {
        let next_pc = (cpu.regs[15].wrapping_sub(2)) | 1;
        cpu.regs[15] = cpu.regs[14].wrapping_add(offset << 1);
        cpu.regs[14] = next_pc;
        cpu.pipeline_valid = false;
        3
    }
}

fn multiply_cycles_thumb(val: u32) -> u32 {
    if val & 0xFFFF_FF00 == 0 || val & 0xFFFF_FF00 == 0xFFFF_FF00 { 1 }
    else if val & 0xFFFF_0000 == 0 || val & 0xFFFF_0000 == 0xFFFF_0000 { 2 }
    else if val & 0xFF00_0000 == 0 || val & 0xFF00_0000 == 0xFF00_0000 { 3 }
    else { 4 }
}
