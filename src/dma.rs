pub struct Dma {
    control: u32,
    irq_en: bool,
    channel_irq_en: u8,
    channel_irq_flags: u8,
    force_irq: bool,
    irq_dummy: u8,

    channels: [Channel; 7],
}

impl Dma {
    pub fn new() -> Dma {
        Dma {
            control: 0x07654321,
            irq_en: false,
            channel_irq_en: 0,
            channel_irq_flags: 0,
            force_irq: false,
            irq_dummy: 0,
            channels: [
                Channel::new(),
                Channel::new(),
                Channel::new(),
                Channel::new(),
                Channel::new(),
                Channel::new(),
                Channel::new(),
            ],
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

    pub fn channel(&self, port: Port) -> &Channel {
        &self.channels[port as usize]
    }

    pub fn channel_mut(&mut self, port: Port) -> &mut Channel {
        &mut self.channels[port as usize]
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Port {
    MdecIn = 0,
    MdecOut = 1,
    Gpu = 2,
    CdRom = 3,
    Spu = 4,
    Pio = 5,
    Otc = 6,
}

impl Port {
    pub fn from_index(index: u32) -> Port {
        match index {
            0 => Port::MdecIn,
            1 => Port::MdecOut,
            2 => Port::Gpu,
            3 => Port::CdRom,
            4 => Port::Spu,
            5 => Port::Pio,
            6 => Port::Otc,
            n => panic!("Invalid port {}", n),
        }
    }
}

pub struct Channel {
    enable: bool,
    direction: Direction,
    step: Step,
    sync: Sync,
    trigger: bool,
    chop: bool,
    chop_dma_sz: u8,
    chop_cpu_sz: u8,
    dummy: u8,

    base: u32,
    block_size: u16,
    block_count: u16,
}

impl Channel {
    fn new() -> Channel {
        Channel {
            enable: false,
            direction: Direction::ToRam,
            step: Step::Increment,
            sync: Sync::Manual,
            trigger: false,
            chop: false,
            chop_dma_sz: 0,
            chop_cpu_sz: 0,
            dummy: 0,

            base: 0,
            block_size: 0,
            block_count: 0,
        }
    }

    pub fn active(&self) -> bool {
        let trigger = match self.sync {
            Sync::Manual => self.trigger,
            _ => true,
        };

        self.enable && trigger
    }

    pub fn sync(&self) -> Sync {
        self.sync
    }

    pub fn step(&self) -> Step {
        self.step
    }

    pub fn direction(&self) -> Direction {
        self.direction
    }

    pub fn transfer_size(&self) -> Option<u32> {
        let bs = self.block_size as u32;
        let bc = self.block_count as u32;

        match self.sync {
            Sync::Manual => Some(bs),
            Sync::Request => Some(bc * bs),
            // LinkedListモードでは事前にサイズが分からない
            Sync::LinkedList => None,
        }
    }

    pub fn done(&mut self) {
        self.enable = false;
        self.trigger = false;
    }

    pub fn base(&self) -> u32 {
        self.base
    }

    pub fn set_base(&mut self, val: u32) {
        self.base = val & 0xFFFFFF;
    }

    pub fn control(&self) -> u32 {
        let mut r = 0;

        r |= (self.direction as u32) << 0;
        r |= (self.step as u32) << 1;
        r |= (self.chop as u32) << 8;
        r |= (self.sync as u32) << 9;
        r |= (self.chop_dma_sz as u32) << 16;
        r |= (self.chop_cpu_sz as u32) << 20;
        r |= (self.enable as u32) << 24;
        r |= (self.trigger as u32) << 28;
        r |= (self.dummy as u32) << 29;

        r
    }

    pub fn set_control(&mut self, val: u32) {
        self.direction = match val & 1 != 0 {
            true => Direction::FromRam,
            false => Direction::ToRam,
        };

        self.step = match (val >> 1) & 1 != 0 {
            true => Step::Decrement,
            false => Step::Increment,
        };

        self.chop = (val >> 8) & 1 != 0;

        self.sync = match (val >> 9) & 3 {
            0 => Sync::Manual,
            1 => Sync::Request,
            2 => Sync::LinkedList,
            n => panic!("Unknown DMA sync mode {}", n),
        };

        self.chop_dma_sz = ((val >> 16) & 7) as u8;
        self.chop_cpu_sz = ((val >> 20) & 7) as u8;

        self.enable = (val >> 24) & 1 != 0;
        self.trigger = (val >> 28) & 1 != 0;

        self.dummy = ((val >> 29) & 3) as u8;
    }

    pub fn block_control(&self) -> u32 {
        let bs = self.block_size as u32;
        let bc = self.block_count as u32;

        (bc << 16) | bs
    }

    pub fn set_block_control(&mut self, val: u32) {
        self.block_size = val as u16;
        self.block_count = (val >> 16) as u16;
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Direction {
    ToRam = 0,
    FromRam = 1,
}

#[derive(Clone, Copy)]
pub enum Step {
    Increment = 0,
    Decrement = 1,
}

#[derive(Clone, Copy)]
pub enum Sync {
    Manual = 0,
    Request = 1,
    LinkedList = 2,
}
