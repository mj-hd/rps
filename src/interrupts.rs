use crate::addressible::Addressible;

pub enum Irq {
    VBlank = 0,
    Gpu = 1,
    CdRom = 2,
    Dma = 3,
    Tmr0 = 4,
    Tmr1 = 5,
    Tmr2 = 6,
    ControllerMemoryCard = 7,
    Sio = 8,
    Spu = 9,
    LightPen = 10,
}

pub struct Interrupts {
    stat: u32,
    mask: u32,

    prev_stat: u32,
}

impl Interrupts {
    pub fn new() -> Self {
        Self {
            stat: 0,
            mask: 0,
            prev_stat: 0,
        }
    }

    pub fn load<T: Addressible>(&self, offset: u32) -> T {
        let res = match offset {
            0 => self.stat,
            4 => self.mask,
            _ => unreachable!(),
        };

        Addressible::from_u32(res)
    }

    pub fn store<T: Addressible>(&mut self, offset: u32, val: T) {
        let val = val.as_u32();

        match offset {
            0 => self.ack(val),
            4 => self.mask = val,
            _ => unreachable!(),
        }
    }

    pub fn tick(&mut self) {
        self.prev_stat = self.stat;
    }

    pub fn check(&mut self) -> bool {
        let irq = self.stat & self.mask;

        irq != 0
    }

    fn ack(&mut self, val: u32) {
        self.stat &= !val;
    }

    pub fn raise(&mut self, irq: Irq) {
        let mask = 1 << (irq as u32);

        if self.prev_stat & mask == 0 {
            self.stat |= mask;
        }
    }
}
