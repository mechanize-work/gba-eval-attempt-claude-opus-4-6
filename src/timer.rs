use crate::apu::Apu;

pub struct Timer {
    pub counter: u16,
    pub reload: u16,
    pub control: u16,
    pub enabled: bool,
    pub cascade: bool,
    pub prescaler: u32,
    pub internal_counter: u32,
    pub irq: bool,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            counter: 0,
            reload: 0,
            control: 0,
            enabled: false,
            cascade: false,
            prescaler: 1,
            internal_counter: 0,
            irq: false,
        }
    }
}

pub struct Timers {
    pub timers: [Timer; 4],
}

impl Timers {
    pub fn new() -> Self {
        Self {
            timers: [Timer::new(), Timer::new(), Timer::new(), Timer::new()],
        }
    }

    pub fn read_counter(&self, idx: usize) -> u16 {
        self.timers[idx].counter
    }

    pub fn read_control(&self, idx: usize) -> u16 {
        self.timers[idx].control
    }

    pub fn write_reload(&mut self, idx: usize, val: u16) {
        self.timers[idx].reload = val;
    }

    pub fn write_control(&mut self, idx: usize, val: u16) {
        let was_enabled = self.timers[idx].enabled;
        self.timers[idx].control = val;
        self.timers[idx].enabled = val & (1 << 7) != 0;
        self.timers[idx].cascade = val & (1 << 2) != 0 && idx > 0;
        self.timers[idx].irq = val & (1 << 6) != 0;
        self.timers[idx].prescaler = match val & 3 {
            0 => 1,
            1 => 64,
            2 => 256,
            3 => 1024,
            _ => 1,
        };

        if !was_enabled && self.timers[idx].enabled {
            self.timers[idx].counter = self.timers[idx].reload;
            self.timers[idx].internal_counter = 0;
        }
    }

    pub fn tick(&mut self, cycles: u32, apu: &mut Apu, if_: &mut u16) {
        let mut overflow = [false; 4];

        for i in 0..4 {
            if !self.timers[i].enabled { continue; }
            if self.timers[i].cascade { continue; }

            self.timers[i].internal_counter += cycles;
            let prescaler = self.timers[i].prescaler;

            while self.timers[i].internal_counter >= prescaler {
                self.timers[i].internal_counter -= prescaler;
                let (new_val, overflowed) = self.timers[i].counter.overflowing_add(1);
                if overflowed {
                    self.timers[i].counter = self.timers[i].reload;
                    overflow[i] = true;
                    if self.timers[i].irq {
                        *if_ |= 1 << (3 + i);
                    }
                } else {
                    self.timers[i].counter = new_val;
                }
            }
        }

        for i in 1..4 {
            if !self.timers[i].enabled { continue; }
            if !self.timers[i].cascade { continue; }
            if !overflow[i - 1] { continue; }

            let (new_val, overflowed) = self.timers[i].counter.overflowing_add(1);
            if overflowed {
                self.timers[i].counter = self.timers[i].reload;
                overflow[i] = true;
                if self.timers[i].irq {
                    *if_ |= 1 << (3 + i);
                }
            } else {
                self.timers[i].counter = new_val;
            }
        }

        let mut dma_request = [false; 2];
        for i in 0..2 {
            if overflow[i] {
                apu.on_timer_overflow(i, &mut dma_request);
            }
        }
    }
}
