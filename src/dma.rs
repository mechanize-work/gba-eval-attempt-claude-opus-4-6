#[derive(Clone, Copy, PartialEq)]
pub enum DmaTrigger {
    Immediate,
    VBlank,
    HBlank,
    Special,
}

pub struct DmaChannel {
    pub src: u32,
    pub dst: u32,
    pub count: u16,
    pub cnt: u16,
    pub internal_src: u32,
    pub internal_dst: u32,
    pub internal_count: u32,
    pub active: bool,
}

impl DmaChannel {
    pub fn new() -> Self {
        Self {
            src: 0, dst: 0, count: 0, cnt: 0,
            internal_src: 0, internal_dst: 0, internal_count: 0,
            active: false,
        }
    }
}

pub struct DmaController {
    pub channels: [DmaChannel; 4],
}

impl DmaController {
    pub fn new() -> Self {
        Self {
            channels: [DmaChannel::new(), DmaChannel::new(), DmaChannel::new(), DmaChannel::new()],
        }
    }

    pub fn any_active(&self) -> bool {
        self.channels.iter().any(|c| c.active)
    }

    pub fn read_reg(&self, addr: u32) -> u16 {
        let ch = match addr {
            0x0B0..=0x0BB => 0,
            0x0BC..=0x0C7 => 1,
            0x0C8..=0x0D3 => 2,
            0x0D4..=0x0DF => 3,
            _ => return 0,
        };
        let base = 0x0B0 + ch as u32 * 12;
        let offset = addr - base;
        let c = &self.channels[ch];
        match offset {
            0 => c.src as u16,
            2 => (c.src >> 16) as u16,
            4 => c.dst as u16,
            6 => (c.dst >> 16) as u16,
            8 => c.count,
            10 => c.cnt,
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, addr: u32, val: u16) {
        let ch = match addr {
            0x0B0..=0x0BB => 0,
            0x0BC..=0x0C7 => 1,
            0x0C8..=0x0D3 => 2,
            0x0D4..=0x0DF => 3,
            _ => return,
        };
        let base = 0x0B0 + ch as u32 * 12;
        let offset = addr - base;
        let c = &mut self.channels[ch];
        match offset {
            0 => c.src = (c.src & 0xFFFF0000) | val as u32,
            2 => c.src = (c.src & 0x0000FFFF) | ((val as u32) << 16),
            4 => c.dst = (c.dst & 0xFFFF0000) | val as u32,
            6 => c.dst = (c.dst & 0x0000FFFF) | ((val as u32) << 16),
            8 => c.count = val,
            10 => c.cnt = val,
            _ => {}
        }
    }

    pub fn check_enable(&mut self, ch: usize) {
        let c = &mut self.channels[ch];
        if c.cnt & (1 << 15) != 0 {
            let src_mask = if ch == 0 { 0x07FF_FFFF } else { 0x0FFF_FFFF };
            let dst_mask = if ch == 3 { 0x0FFF_FFFF } else { 0x07FF_FFFF };

            c.internal_src = c.src & src_mask;
            c.internal_dst = c.dst & dst_mask;
            let max_count = if ch == 3 { 0x10000u32 } else { 0x4000 };
            c.internal_count = if c.count == 0 { max_count } else { c.count as u32 };

            let timing = (c.cnt >> 12) & 3;
            if timing == 0 {
                c.active = true;
            }
        }
    }

    pub fn trigger(&mut self, trigger: DmaTrigger) {
        for ch in 0..4 {
            let c = &mut self.channels[ch];
            if c.cnt & (1 << 15) == 0 { continue; }

            let timing = (c.cnt >> 12) & 3;
            let matches = match timing {
                1 => trigger == DmaTrigger::VBlank,
                2 => trigger == DmaTrigger::HBlank,
                3 => trigger == DmaTrigger::Special,
                _ => false,
            };

            if matches {
                c.active = true;
                if c.internal_count == 0 {
                    let max_count = if ch == 3 { 0x10000u32 } else { 0x4000 };
                    c.internal_count = if c.count == 0 { max_count } else { c.count as u32 };
                }
            }
        }
    }

    pub fn trigger_sound_fifo(&mut self, fifo: usize) {
        let ch = if fifo == 0 { 1 } else { 2 };
        let c = &mut self.channels[ch];
        if c.cnt & (1 << 15) == 0 { return; }
        let timing = (c.cnt >> 12) & 3;
        if timing == 3 {
            c.active = true;
            c.internal_count = 4;
        }
    }
}
