use crate::timer::Timers;

pub struct Apu {
    pub enabled: bool,

    pub ch1_sweep: u8,
    pub ch1_duty_len: u8,
    pub ch1_envelope: u8,
    pub ch1_freq: u16,
    pub ch1_timer: u32,
    pub ch1_freq_counter: u32,
    pub ch1_duty_pos: u8,
    pub ch1_env_timer: u8,
    pub ch1_volume: u8,
    pub ch1_enabled: bool,
    pub ch1_len_counter: u8,
    pub ch1_sweep_timer: u8,
    pub ch1_shadow_freq: u16,
    pub ch1_sweep_enabled: bool,

    pub ch2_duty_len: u8,
    pub ch2_envelope: u8,
    pub ch2_freq: u16,
    pub ch2_timer: u32,
    pub ch2_freq_counter: u32,
    pub ch2_duty_pos: u8,
    pub ch2_env_timer: u8,
    pub ch2_volume: u8,
    pub ch2_enabled: bool,
    pub ch2_len_counter: u8,

    pub ch3_enable: u8,
    pub ch3_len: u8,
    pub ch3_volume: u8,
    pub ch3_freq: u16,
    pub ch3_timer: u32,
    pub ch3_freq_counter: u32,
    pub ch3_pos: u8,
    pub ch3_enabled: bool,
    pub ch3_len_counter: u16,
    pub wave_ram: [u8; 16],
    pub ch3_bank: u8,
    pub ch3_dimension: bool,

    pub ch4_len_env: u8,
    pub ch4_envelope: u8,
    pub ch4_poly: u8,
    pub ch4_control: u8,
    pub ch4_timer: u32,
    pub ch4_freq_counter: u32,
    pub ch4_lfsr: u16,
    pub ch4_env_timer: u8,
    pub ch4_volume: u8,
    pub ch4_enabled: bool,
    pub ch4_len_counter: u8,

    pub soundcnt_l: u16,
    pub soundcnt_h: u16,
    pub soundcnt_x: u16,
    pub soundbias: u16,

    pub fifo_a: [i8; 32],
    pub fifo_a_read: usize,
    pub fifo_a_write: usize,
    pub fifo_a_count: usize,
    pub fifo_a_sample: i8,

    pub fifo_b: [i8; 32],
    pub fifo_b_read: usize,
    pub fifo_b_write: usize,
    pub fifo_b_count: usize,
    pub fifo_b_sample: i8,

    pub frame_seq_counter: u32,
    pub frame_seq_step: u8,
}

const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 0],
];

impl Apu {
    pub fn new() -> Self {
        Self {
            enabled: false,
            ch1_sweep: 0, ch1_duty_len: 0, ch1_envelope: 0, ch1_freq: 0,
            ch1_timer: 0, ch1_freq_counter: 0, ch1_duty_pos: 0,
            ch1_env_timer: 0, ch1_volume: 0, ch1_enabled: false, ch1_len_counter: 0,
            ch1_sweep_timer: 0, ch1_shadow_freq: 0, ch1_sweep_enabled: false,
            ch2_duty_len: 0, ch2_envelope: 0, ch2_freq: 0,
            ch2_timer: 0, ch2_freq_counter: 0, ch2_duty_pos: 0,
            ch2_env_timer: 0, ch2_volume: 0, ch2_enabled: false, ch2_len_counter: 0,
            ch3_enable: 0, ch3_len: 0, ch3_volume: 0, ch3_freq: 0,
            ch3_timer: 0, ch3_freq_counter: 0, ch3_pos: 0,
            ch3_enabled: false, ch3_len_counter: 0,
            wave_ram: [0; 16], ch3_bank: 0, ch3_dimension: false,
            ch4_len_env: 0, ch4_envelope: 0, ch4_poly: 0, ch4_control: 0,
            ch4_timer: 0, ch4_freq_counter: 0, ch4_lfsr: 0x7FFF,
            ch4_env_timer: 0, ch4_volume: 0, ch4_enabled: false, ch4_len_counter: 0,
            soundcnt_l: 0, soundcnt_h: 0, soundcnt_x: 0, soundbias: 0x200,
            fifo_a: [0; 32], fifo_a_read: 0, fifo_a_write: 0, fifo_a_count: 0, fifo_a_sample: 0,
            fifo_b: [0; 32], fifo_b_read: 0, fifo_b_write: 0, fifo_b_count: 0, fifo_b_sample: 0,
            frame_seq_counter: 0, frame_seq_step: 0,
        }
    }

    pub fn read_reg(&self, addr: u32) -> u16 {
        match addr {
            0x060 => self.ch1_sweep as u16,
            0x062 => self.ch1_duty_len as u16 & 0xC0,
            0x064 => self.ch1_freq & 0xC7FF,
            0x068 => self.ch2_duty_len as u16 & 0xC0,
            0x06C => self.ch2_freq & 0xC7FF,
            0x070 => self.ch3_enable as u16,
            0x072 => self.ch3_volume as u16,
            0x074 => self.ch3_freq & 0xC7FF,
            0x078 => self.ch4_len_env as u16,
            0x07C => self.ch4_poly as u16 | ((self.ch4_control as u16) << 8),
            0x080 => self.soundcnt_l,
            0x082 => self.soundcnt_h,
            0x084 => {
                let mut val = self.soundcnt_x & 0x80;
                if self.ch1_enabled { val |= 1; }
                if self.ch2_enabled { val |= 2; }
                if self.ch3_enabled { val |= 4; }
                if self.ch4_enabled { val |= 8; }
                val
            }
            0x088 => self.soundbias,
            0x090..=0x09E => {
                let idx = ((addr - 0x090) & 0xF) as usize;
                if idx + 1 < self.wave_ram.len() {
                    u16::from_le_bytes([self.wave_ram[idx], self.wave_ram[idx + 1]])
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    pub fn write_reg(&mut self, addr: u32, val: u16) {
        match addr {
            0x060 => self.ch1_sweep = val as u8,
            0x062 => {
                self.ch1_duty_len = val as u8;
                self.ch1_envelope = (val >> 8) as u8;
            }
            0x064 => {
                self.ch1_freq = val;
                if val & 0x8000 != 0 {
                    self.trigger_ch1();
                }
            }
            0x068 => {
                self.ch2_duty_len = val as u8;
                self.ch2_envelope = (val >> 8) as u8;
            }
            0x06C => {
                self.ch2_freq = val;
                if val & 0x8000 != 0 {
                    self.trigger_ch2();
                }
            }
            0x070 => {
                self.ch3_enable = val as u8;
                let bank = (val >> 6) & 1;
                self.ch3_dimension = val & (1 << 5) != 0;
                self.ch3_bank = bank as u8;
                if val & 0x80 == 0 {
                    self.ch3_enabled = false;
                }
            }
            0x072 => {
                self.ch3_len = val as u8;
                self.ch3_volume = (val >> 8) as u8;
            }
            0x074 => {
                self.ch3_freq = val;
                if val & 0x8000 != 0 {
                    self.trigger_ch3();
                }
            }
            0x078 => {
                self.ch4_len_env = val as u8;
                self.ch4_envelope = (val >> 8) as u8;
            }
            0x07C => {
                self.ch4_poly = val as u8;
                self.ch4_control = (val >> 8) as u8;
                if val & 0x8000 != 0 {
                    self.trigger_ch4();
                }
            }
            0x080 => self.soundcnt_l = val,
            0x082 => {
                self.soundcnt_h = val;
                if val & (1 << 11) != 0 {
                    self.fifo_a_read = 0;
                    self.fifo_a_write = 0;
                    self.fifo_a_count = 0;
                }
                if val & (1 << 15) != 0 {
                    self.fifo_b_read = 0;
                    self.fifo_b_write = 0;
                    self.fifo_b_count = 0;
                }
            }
            0x084 => {
                let new_enabled = val & 0x80 != 0;
                if !new_enabled && self.enabled {
                    self.ch1_enabled = false;
                    self.ch2_enabled = false;
                    self.ch3_enabled = false;
                    self.ch4_enabled = false;
                }
                self.enabled = new_enabled;
                self.soundcnt_x = val & 0x80;
            }
            0x088 => self.soundbias = val,
            0x090..=0x09E => {
                let idx = ((addr - 0x090) & 0xF) as usize;
                let bytes = val.to_le_bytes();
                if idx < self.wave_ram.len() { self.wave_ram[idx] = bytes[0]; }
                if idx + 1 < self.wave_ram.len() { self.wave_ram[idx + 1] = bytes[1]; }
            }
            0x0A0 => self.write_fifo_a(val as u32),
            0x0A2 => self.write_fifo_a((val as u32) << 16),
            0x0A4 => self.write_fifo_b(val as u32),
            0x0A6 => self.write_fifo_b((val as u32) << 16),
            _ => {}
        }
    }

    pub fn write_fifo_a(&mut self, val: u32) {
        for i in 0..4 {
            if self.fifo_a_count < 32 {
                self.fifo_a[self.fifo_a_write] = (val >> (i * 8)) as i8;
                self.fifo_a_write = (self.fifo_a_write + 1) % 32;
                self.fifo_a_count += 1;
            }
        }
    }

    pub fn write_fifo_b(&mut self, val: u32) {
        for i in 0..4 {
            if self.fifo_b_count < 32 {
                self.fifo_b[self.fifo_b_write] = (val >> (i * 8)) as i8;
                self.fifo_b_write = (self.fifo_b_write + 1) % 32;
                self.fifo_b_count += 1;
            }
        }
    }

    pub fn on_timer_overflow(&mut self, timer_id: usize, dma_request: &mut [bool; 2]) {
        let fifo_a_timer = (self.soundcnt_h >> 10) & 1;
        let fifo_b_timer = (self.soundcnt_h >> 14) & 1;

        if timer_id == fifo_a_timer as usize {
            if self.fifo_a_count > 0 {
                self.fifo_a_sample = self.fifo_a[self.fifo_a_read];
                self.fifo_a_read = (self.fifo_a_read + 1) % 32;
                self.fifo_a_count -= 1;
            }
            if self.fifo_a_count <= 16 {
                dma_request[0] = true;
            }
        }

        if timer_id == fifo_b_timer as usize {
            if self.fifo_b_count > 0 {
                self.fifo_b_sample = self.fifo_b[self.fifo_b_read];
                self.fifo_b_read = (self.fifo_b_read + 1) % 32;
                self.fifo_b_count -= 1;
            }
            if self.fifo_b_count <= 16 {
                dma_request[1] = true;
            }
        }
    }

    fn trigger_ch1(&mut self) {
        self.ch1_enabled = true;
        self.ch1_len_counter = if self.ch1_duty_len & 0x3F == 0 { 64 } else { 64 - (self.ch1_duty_len & 0x3F) };
        let freq = self.ch1_freq & 0x7FF;
        self.ch1_freq_counter = (2048 - freq as u32) * 16;
        self.ch1_timer = self.ch1_freq_counter;
        self.ch1_volume = self.ch1_envelope >> 4;
        self.ch1_env_timer = self.ch1_envelope & 7;
        self.ch1_shadow_freq = freq;
        let sweep_period = (self.ch1_sweep >> 4) & 7;
        let sweep_shift = self.ch1_sweep & 7;
        self.ch1_sweep_timer = if sweep_period == 0 { 8 } else { sweep_period };
        self.ch1_sweep_enabled = sweep_period != 0 || sweep_shift != 0;
    }

    fn trigger_ch2(&mut self) {
        self.ch2_enabled = true;
        self.ch2_len_counter = if self.ch2_duty_len & 0x3F == 0 { 64 } else { 64 - (self.ch2_duty_len & 0x3F) };
        let freq = self.ch2_freq & 0x7FF;
        self.ch2_freq_counter = (2048 - freq as u32) * 16;
        self.ch2_timer = self.ch2_freq_counter;
        self.ch2_volume = self.ch2_envelope >> 4;
        self.ch2_env_timer = self.ch2_envelope & 7;
    }

    fn trigger_ch3(&mut self) {
        self.ch3_enabled = true;
        self.ch3_len_counter = if self.ch3_len == 0 { 256 } else { 256 - self.ch3_len as u16 };
        let freq = self.ch3_freq & 0x7FF;
        self.ch3_freq_counter = (2048 - freq as u32) * 8;
        self.ch3_timer = self.ch3_freq_counter;
        self.ch3_pos = 0;
    }

    fn trigger_ch4(&mut self) {
        self.ch4_enabled = true;
        self.ch4_len_counter = if self.ch4_len_env & 0x3F == 0 { 64 } else { 64 - (self.ch4_len_env & 0x3F) };
        self.ch4_lfsr = 0x7FFF;
        self.ch4_volume = self.ch4_envelope >> 4;
        self.ch4_env_timer = self.ch4_envelope & 7;

        let r = (self.ch4_poly & 7) as u32;
        let s = ((self.ch4_poly >> 4) & 0xF) as u32;
        self.ch4_freq_counter = if r == 0 { 8 << s } else { (16 * r) << s };
        self.ch4_timer = self.ch4_freq_counter;
    }

    pub fn tick_frame_sequencer(&mut self) {
        self.frame_seq_counter += 1;
        if self.frame_seq_counter >= 32768 / 512 {
            self.frame_seq_counter = 0;
            self.frame_seq_step = (self.frame_seq_step + 1) & 7;

            if self.frame_seq_step & 1 == 0 {
                self.clock_length();
            }
            if self.frame_seq_step == 7 {
                self.clock_envelope();
            }
            if self.frame_seq_step == 2 || self.frame_seq_step == 6 {
                self.clock_sweep();
            }
        }
    }

    fn clock_length(&mut self) {
        if self.ch1_freq & (1 << 14) != 0 && self.ch1_len_counter > 0 {
            self.ch1_len_counter -= 1;
            if self.ch1_len_counter == 0 { self.ch1_enabled = false; }
        }
        if self.ch2_freq & (1 << 14) != 0 && self.ch2_len_counter > 0 {
            self.ch2_len_counter -= 1;
            if self.ch2_len_counter == 0 { self.ch2_enabled = false; }
        }
        if self.ch3_freq & (1 << 14) != 0 && self.ch3_len_counter > 0 {
            self.ch3_len_counter -= 1;
            if self.ch3_len_counter == 0 { self.ch3_enabled = false; }
        }
        if self.ch4_control & (1 << 6) != 0 && self.ch4_len_counter > 0 {
            self.ch4_len_counter -= 1;
            if self.ch4_len_counter == 0 { self.ch4_enabled = false; }
        }
    }

    fn clock_envelope(&mut self) {
        if self.ch1_enabled {
            let period = self.ch1_envelope & 7;
            if period != 0 {
                if self.ch1_env_timer > 0 { self.ch1_env_timer -= 1; }
                if self.ch1_env_timer == 0 {
                    self.ch1_env_timer = period;
                    if self.ch1_envelope & 8 != 0 {
                        if self.ch1_volume < 15 { self.ch1_volume += 1; }
                    } else {
                        if self.ch1_volume > 0 { self.ch1_volume -= 1; }
                    }
                }
            }
        }
        if self.ch2_enabled {
            let period = self.ch2_envelope & 7;
            if period != 0 {
                if self.ch2_env_timer > 0 { self.ch2_env_timer -= 1; }
                if self.ch2_env_timer == 0 {
                    self.ch2_env_timer = period;
                    if self.ch2_envelope & 8 != 0 {
                        if self.ch2_volume < 15 { self.ch2_volume += 1; }
                    } else {
                        if self.ch2_volume > 0 { self.ch2_volume -= 1; }
                    }
                }
            }
        }
        if self.ch4_enabled {
            let period = self.ch4_envelope & 7;
            if period != 0 {
                if self.ch4_env_timer > 0 { self.ch4_env_timer -= 1; }
                if self.ch4_env_timer == 0 {
                    self.ch4_env_timer = period;
                    if self.ch4_envelope & 8 != 0 {
                        if self.ch4_volume < 15 { self.ch4_volume += 1; }
                    } else {
                        if self.ch4_volume > 0 { self.ch4_volume -= 1; }
                    }
                }
            }
        }
    }

    fn clock_sweep(&mut self) {
        if !self.ch1_sweep_enabled { return; }
        let period = (self.ch1_sweep >> 4) & 7;
        if period == 0 { return; }

        if self.ch1_sweep_timer > 0 { self.ch1_sweep_timer -= 1; }
        if self.ch1_sweep_timer == 0 {
            self.ch1_sweep_timer = period;
            let shift = self.ch1_sweep & 7;
            if shift != 0 {
                let delta = self.ch1_shadow_freq >> shift;
                let new_freq = if self.ch1_sweep & 8 != 0 {
                    self.ch1_shadow_freq.wrapping_sub(delta)
                } else {
                    self.ch1_shadow_freq + delta
                };
                if new_freq > 2047 {
                    self.ch1_enabled = false;
                } else {
                    self.ch1_shadow_freq = new_freq;
                    self.ch1_freq = (self.ch1_freq & !0x7FF) | (new_freq & 0x7FF);
                    self.ch1_freq_counter = (2048 - new_freq as u32) * 16;
                }
            }
        }
    }

    pub fn generate_sample(&mut self, _timers: &Timers) -> (i16, i16) {
        if !self.enabled {
            return (0, 0);
        }

        self.tick_frame_sequencer();

        let mut psg_left = 0i32;
        let mut psg_right = 0i32;

        let left_vol = ((self.soundcnt_l >> 4) & 7) as i32 + 1;
        let right_vol = (self.soundcnt_l & 7) as i32 + 1;
        let left_enable = (self.soundcnt_l >> 12) & 0xF;
        let right_enable = (self.soundcnt_l >> 8) & 0xF;

        let ch1_out = self.sample_ch1();
        let ch2_out = self.sample_ch2();
        let ch3_out = self.sample_ch3();
        let ch4_out = self.sample_ch4();

        if left_enable & 1 != 0 { psg_left += ch1_out; }
        if left_enable & 2 != 0 { psg_left += ch2_out; }
        if left_enable & 4 != 0 { psg_left += ch3_out; }
        if left_enable & 8 != 0 { psg_left += ch4_out; }

        if right_enable & 1 != 0 { psg_right += ch1_out; }
        if right_enable & 2 != 0 { psg_right += ch2_out; }
        if right_enable & 4 != 0 { psg_right += ch3_out; }
        if right_enable & 8 != 0 { psg_right += ch4_out; }

        psg_left = psg_left * left_vol / 8;
        psg_right = psg_right * right_vol / 8;

        let psg_volume = match self.soundcnt_h & 3 {
            0 => 1,
            1 => 2,
            2 => 4,
            _ => 0,
        };
        psg_left = psg_left * psg_volume / 4;
        psg_right = psg_right * psg_volume / 4;

        let fifo_a_vol = if self.soundcnt_h & (1 << 2) != 0 { 4 } else { 2 };
        let fifo_b_vol = if self.soundcnt_h & (1 << 3) != 0 { 4 } else { 2 };

        let fifo_a = self.fifo_a_sample as i32 * fifo_a_vol;
        let fifo_b = self.fifo_b_sample as i32 * fifo_b_vol;

        let mut left = psg_left;
        let mut right = psg_right;

        if self.soundcnt_h & (1 << 9) != 0 { left += fifo_a; }
        if self.soundcnt_h & (1 << 8) != 0 { right += fifo_a; }
        if self.soundcnt_h & (1 << 13) != 0 { left += fifo_b; }
        if self.soundcnt_h & (1 << 12) != 0 { right += fifo_b; }

        let bias = (self.soundbias & 0x3FE) as i32;
        left = ((left + bias).max(0).min(0x3FF) - bias) * 32;
        right = ((right + bias).max(0).min(0x3FF) - bias) * 32;

        (left.max(-32768).min(32767) as i16, right.max(-32768).min(32767) as i16)
    }

    fn sample_ch1(&mut self) -> i32 {
        if !self.ch1_enabled { return 0; }
        let duty = (self.ch1_duty_len >> 6) as usize;
        let sample = DUTY_TABLE[duty][self.ch1_duty_pos as usize];
        if sample != 0 { self.ch1_volume as i32 } else { -(self.ch1_volume as i32) }
    }

    fn sample_ch2(&mut self) -> i32 {
        if !self.ch2_enabled { return 0; }
        let duty = (self.ch2_duty_len >> 6) as usize;
        let sample = DUTY_TABLE[duty][self.ch2_duty_pos as usize];
        if sample != 0 { self.ch2_volume as i32 } else { -(self.ch2_volume as i32) }
    }

    fn sample_ch3(&mut self) -> i32 {
        if !self.ch3_enabled { return 0; }
        let pos = self.ch3_pos as usize;
        let byte = self.wave_ram[pos / 2];
        let sample = if pos & 1 == 0 { byte >> 4 } else { byte & 0xF };

        let vol_shift = match (self.ch3_volume >> 5) & 3 {
            0 => 4,
            1 => 0,
            2 => 1,
            3 => 2,
            _ => 4,
        };

        let out = (sample >> vol_shift) as i32;
        out - 8
    }

    fn sample_ch4(&mut self) -> i32 {
        if !self.ch4_enabled { return 0; }
        let bit = (self.ch4_lfsr & 1) ^ 1;
        if bit != 0 { self.ch4_volume as i32 } else { -(self.ch4_volume as i32) }
    }
}
