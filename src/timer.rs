use log::debug;

use crate::addressible::Addressible;

pub struct Timer {
    index: u8,
    counter: u16,
    internal_counter: u32,

    sync_enable: bool,
    sync_mode: u8,
    use_target: bool,
    irq_target: bool,
    irq_full: bool,
    irq_repeat: bool,
    irq_toggle: bool,
    clock_source: u8,

    pub n_irq: bool,
    raised: bool,
    prev_hblank: bool,
    prev_vblank: bool,

    target: u16,
}

impl Timer {
    pub fn new(index: u8) -> Self {
        Self {
            index,
            counter: 0,
            internal_counter: 0,
            sync_enable: false,
            sync_mode: 0,
            use_target: false,
            irq_target: false,
            irq_full: false,
            irq_repeat: false,
            irq_toggle: false,
            clock_source: 0,
            target: 0,
            n_irq: true,
            raised: false,
            prev_hblank: false,
            prev_vblank: false,
        }
    }

    pub fn load<T: Addressible>(&self, offset: u32) -> T {
        match offset {
            0 => Addressible::from_u32(self.counter as u32),
            4 => Addressible::from_u32(self.mode()),
            8 => Addressible::from_u32(self.target as u32),
            _ => unreachable!(),
        }
    }

    pub fn store<T: Addressible>(&mut self, offset: u32, val: T) {
        match offset {
            0 => {
                debug!("TIMER{} set counter {:08x}", self.index, val.as_u32());
                self.counter = val.as_u32() as u16;
            }
            4 => self.set_mode(val.as_u32()),
            8 => {
                debug!("TIMER{} set target {:04x}", self.index, val.as_u32() as u16);
                self.target = val.as_u32() as u16;
            }
            _ => unreachable!(),
        }
    }

    pub fn tick(&mut self, hblank: bool, vblank: bool, dotclock: bool) {
        let prev_vblank = self.prev_vblank;
        self.prev_vblank = vblank;
        let prev_hblank = self.prev_hblank;
        self.prev_hblank = hblank;

        self.internal_counter = self.internal_counter.wrapping_add(1);

        if self.sync_enable {
            match self.index {
                0 => match self.sync_mode {
                    0 => {
                        if hblank {
                            return;
                        }
                    }
                    1 => {
                        if !prev_hblank && hblank {
                            self.counter = 0;
                        }
                    }
                    2 => {
                        if !prev_hblank && hblank {
                            self.counter = 0;
                        } else if !hblank {
                            return;
                        }
                    }
                    _ => unreachable!(),
                },
                1 => match self.sync_mode {
                    0 => {
                        if vblank {
                            return;
                        }
                    }
                    1 => {
                        if !prev_vblank && vblank {
                            self.counter = 0;
                        }
                    }
                    2 => {
                        if !prev_vblank && vblank {
                            self.counter = 0;
                        } else if !vblank {
                            return;
                        }
                    }
                    _ => unreachable!(),
                },
                2 => match self.sync_mode {
                    0 | 3 => return,
                    1 | 2 => {}
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            }
        }

        let increment = match self.index {
            0 => match self.clock_source {
                0 | 2 => true,
                1 | 3 => dotclock,
                _ => unreachable!(),
            },
            1 => match self.clock_source {
                0 | 2 => true,
                1 | 3 => !prev_hblank && hblank,
                _ => unreachable!(),
            },
            2 => match self.clock_source {
                0 | 1 => true,
                2 | 3 => self.internal_counter % 8 == 0,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };

        if increment {
            self.counter = self.counter.wrapping_add(1);
            if !self.irq_toggle {
                self.n_irq = true;
            }
        }

        if self.counter == self.target {
            if self.irq_target {
                self.raise();
            }
            if self.use_target {
                self.counter = 0;
            }
        }

        if self.counter == 0xFFFF {
            if self.irq_full {
                self.raise();
            }
        }
    }

    fn mode(&self) -> u32 {
        let mut res = self.sync_enable as u32;
        res |= ((self.sync_mode as u32) & 0b11) << 1;
        res |= (self.use_target as u32) << 3;
        res |= (self.irq_target as u32) << 4;
        res |= (self.irq_full as u32) << 5;
        res |= (self.irq_repeat as u32) << 6;
        res |= (self.irq_toggle as u32) << 7;
        res |= ((self.clock_source as u32) & 0b11) << 8;
        res |= (self.n_irq as u32) << 10;
        res |= ((self.counter == self.target) as u32) << 11;
        res |= ((self.counter == 0xFFFF) as u32) << 12;

        res
    }

    fn set_mode(&mut self, val: u32) {
        debug!("TIMER{} set mode {:08x}", self.index, val);
        self.raised = false;
        self.sync_enable = val & 1 != 0;
        self.sync_mode = ((val >> 1) & 0b11) as u8;
        self.use_target = (val >> 3) & 1 != 0;
        self.irq_target = (val >> 4) & 1 != 0;
        self.irq_full = (val >> 5) & 1 != 0;
        self.irq_repeat = (val >> 6) & 1 != 0;
        self.irq_toggle = (val >> 7) & 1 != 0;
        self.clock_source = ((val >> 8) & 0b11) as u8;
        self.n_irq = (val >> 10) & 1 != 0;
    }

    fn raise(&mut self) {
        if self.irq_repeat || !self.raised {
            self.raised = true;
            if self.irq_toggle {
                self.n_irq = !self.n_irq;
                debug!("timer{} irq toggled {}", self.index, !self.n_irq);
            } else {
                self.n_irq = false;
                debug!("timer{} irq raised", self.index);
            }
        }
    }
}
