pub struct Dma {
    control: u32,
    irq_en: bool,
    channel_irq_en: u8,
    channel_irq_flags: u8,
    force_irq: bool,
    irq_dummy: u8,
}

impl Dma {
    pub fn new() -> Dma {
        Dma {
            control: 0x07654321,
        }
    }

    pub fn control(&self) -> u32 {
        self.control
    }

    pub fn set_control(&mut self, val: u32) {
        self.control = val;
    }

    fn irq(&self) -> bool {
        let channel_irq = self.channel_irq_flags & self.channel_irq_en;
        self.force_irq || (self.irq_en && channel_irq != 0)
    }

    pub fn interrupt(&self) -> u32 {
        let mut r = 0;

        r |= self.irq_dummy as u32;
        r |= (self.force_irq as u32) << 15;
        r |= (self.channel_irq_en as u32) << 16;
        r |= (self.irq_en as u32) << 23;
        r |= (self.channel_irq_flags as u32) << 24;
        r |= (self.irq() as u32) << 31;

        r
    }

    pub fn set_interrupt(&mut self, val: u32) {
        self.irq_dummy = (val & 0x3F) as u8;
        self.force_irq = (val >> 15) & 1 != 0;
        self.channel_irq_en = ((val >> 16) & 0x7F) as u8;
        self.irq_en = (val >> 23) & 1 != 0;

        let ack = ((val >> 24) & 0x3F) as u8;
        self.channel_irq_flags &= !ack;
    }
}

struct Channel {
    enable: bool,
    direction: Direction,
    step: Step,
    sync: Sync,
    trigger: bool,
    chop: bool,
    chop_dma_sz: u8,
    chop_cpu_sz: u8,
    dummy: u8,
}
