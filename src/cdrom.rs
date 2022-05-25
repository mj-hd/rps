use std::collections::VecDeque;

use log::warn;

use crate::addressible::{AccessWidth, Addressible};

enum CdRomStatus {
    Idle,
    Seeking,
    Reading,
}

enum CdRomIrq {
    ReadReady = 1,
    SecondOk = 2,
    FirstOk = 3,
    Error = 5,
}

pub struct CdRom {
    index: u8,

    disc: Option<Vec<u8>>,

    parameter_fifo: VecDeque<u8>,
    response_fifo: VecDeque<u8>,
    data_fifo: VecDeque<u8>,

    // stat
    busy: bool,
    status: CdRomStatus,

    // mode
    double_speed: bool,
    raw_sector: bool,

    // request register
    read_active: bool,

    seek_position: Option<Mss>,
    current_position: Mss,
    read_index: u16,

    ie: u8,
    irq: u8,
}

impl CdRom {
    pub fn new(disc: Option<Vec<u8>>) -> Self {
        Self {
            index: 0,
            disc,
            parameter_fifo: VecDeque::with_capacity(16),
            response_fifo: VecDeque::with_capacity(16),
            data_fifo: VecDeque::with_capacity(16),
            busy: false,
            status: CdRomStatus::Idle,
            double_speed: false,
            raw_sector: false,
            read_active: false,
            seek_position: None,
            current_position: Mss {
                min: 0,
                sec: 0,
                sector: 0,
            },
            read_index: 0,
            ie: 0,
            irq: 0,
        }
    }

    pub fn load<T: Addressible>(&mut self, offset: u32) -> T {
        if T::width() != AccessWidth::Byte {
            warn!("CD-ROM invalid load width {:?}", T::width());
            return Addressible::from_u32(0);
        }

        let r = match offset {
            0 => Addressible::from_u32(self.status() as u32),
            1 => self.response_fifo(),
            2 => self.data_fifo(),
            3 => match self.index {
                0 | 2 => self.ie(),
                1 | 3 => self.irq(),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };

        Addressible::from_u32(r as u32)
    }

    pub fn store<T: Addressible>(&mut self, offset: u32, val: T) {
        if T::width() != AccessWidth::Byte {
            warn!("CD-ROM invalid store width {:?}", T::width());
            return;
        }

        let val = val.as_u32() as u8;

        match offset {
            0 => self.set_index(val),
            1 => match self.index {
                0 => self.command(val),
                1 => warn!("Sound Map Data Out"),
                2 => warn!("Sound Map Coding Info"),
                3 => warn!("Audio Volume for Right-Right"),
                _ => unreachable!(),
            },
            2 => match self.index {
                0 => self.set_parameter_fifo(val),
                1 => self.set_ie(val),
                2 => warn!("Audio Volume for Left-Left"),
                3 => warn!("Audio Volume for Right-Left"),
                _ => unreachable!(),
            },
            3 => match self.index {
                0 => self.set_request_register(val),
                1 => self.set_irq(val),
                2 => warn!("Audio Volume for Left-Right"),
                3 => warn!("Audio Volume Apply"),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }

    pub fn check_irq(&self) -> bool {
        let irq = self.irq & self.ie;

        irq != 0
    }

    fn status(&self) -> u8 {
        let mut result = 0;

        result |= self.index & 0b11;
        result |= (self.parameter_fifo.is_empty() as u8) << 3;
        result |= ((self.parameter_fifo.len() < 16) as u8) << 4;
        result |= (!self.response_fifo.is_empty() as u8) << 5;
        result |= (!self.data_fifo.is_empty() as u8) << 6;
        result |= (self.busy as u8) << 7;

        result
    }

    fn set_index(&mut self, val: u8) {
        self.index = val & 0b11;
    }

    fn ie(&self) -> u8 {
        self.ie
    }

    fn set_ie(&mut self, val: u8) {
        self.ie = val;
    }

    fn irq(&self) -> u8 {
        self.irq
    }

    fn set_irq(&mut self, val: u8) {
        self.irq &= !(val & 0x1F);

        if val & 0x40 != 0 {
            self.parameter_fifo.clear();
        }
    }

    fn raise_irq(&mut self, irq: CdRomIrq) {
        self.irq |= 1 << (irq as u8);
    }

    fn set_parameter_fifo(&mut self, val: u8) {
        self.parameter_fifo.push_back(val);
    }

    fn set_request_register(&mut self, val: u8) {
        let prev_active = self.read_active;
        self.read_active = val & 0x80 != 0;

        if !prev_active && self.read_active {
            self.read_index = 0;
        }
    }

    fn response_fifo(&mut self) -> u8 {
        self.response_fifo.pop_front().unwrap_or(0)
    }

    fn data_fifo(&mut self) -> u8 {
        self.data_fifo.pop_front().unwrap_or(0)
    }

    fn stat(&self) -> u8 {
        if self.disc == None {
            0x10 // shell opened
        } else {
            match self.status {
                CdRomStatus::Idle => 0x00,
                CdRomStatus::Seeking => 0x40,
                CdRomStatus::Reading => 0x20,
                _ => 0x02, // motor on
            }
        }
    }

    fn command(&mut self, val: u8) {
        match val {
            0x01 => self.get_stat(),
            0x02 => self.set_loc(),
            0x06 => self.read_n(),
            0x0A => self.init(),
            0x0E => self.set_mode(),
            0x15 => self.seek_l(),
            0x19 => self.test(),
            0x1A => self.get_id(),
            0x1B => self.read_s(),
            _ => {
                warn!("unsupported CD-ROM command {:02x}", val);
                return;
            }
        }

        self.parameter_fifo.clear();
    }

    fn get_stat(&mut self) {
        self.response_fifo.push_back(self.stat());
        self.raise_irq(CdRomIrq::FirstOk);
    }

    fn init(&mut self) {
        self.double_speed = false;
        self.raw_sector = false;
        self.response_fifo.push_back(self.stat());
        self.raise_irq(CdRomIrq::FirstOk);
        // TODO: async gap here
        self.response_fifo.push_back(self.stat());
        self.raise_irq(CdRomIrq::SecondOk);
    }

    fn set_mode(&mut self) {
        let mode = self.parameter_fifo[0];

        self.double_speed = mode & 0x80 != 0;
        self.raw_sector = mode & 0x20 != 0;

        self.response_fifo.push_back(self.stat());
        self.raise_irq(CdRomIrq::FirstOk);
    }

    fn set_loc(&mut self) {
        let addr = Mss {
            min: self.parameter_fifo[0],
            sec: self.parameter_fifo[1],
            sector: self.parameter_fifo[2],
        };

        self.seek_position = Some(addr);

        self.response_fifo.push_back(self.stat());
        self.raise_irq(CdRomIrq::FirstOk);
    }

    fn read_n(&mut self) {
        self.response_fifo.push_back(self.stat());
        self.raise_irq(CdRomIrq::FirstOk);
        // TODO: read then int
    }

    fn read_s(&mut self) {
        self.read_n()
    }

    fn seek_l(&mut self) {
        if let Some(position) = self.seek_position {
            self.current_position = position;
        }

        self.response_fifo.push_back(self.stat());
        self.raise_irq(CdRomIrq::FirstOk);
        // TODO: async gap here
        self.response_fifo.push_back(self.stat());
        self.raise_irq(CdRomIrq::SecondOk);
    }

    fn test(&mut self) {
        match self.parameter_fifo.pop_front() {
            Some(0x20) => self.test_version(),
            Some(n) => warn!("unsupported CD-ROM test func {:02x}", n),
            _ => warn!("CD-ROM test func missing params"),
        }
    }

    fn test_version(&mut self) {
        self.response_fifo.push_back(0x96);
        self.response_fifo.push_back(0x09);
        self.response_fifo.push_back(0x12);
        self.response_fifo.push_back(0xC2);
    }

    fn get_id(&mut self) {
        warn!("get_id")
    }
}

#[derive(Clone, Copy)]
struct Mss {
    min: u8,
    sec: u8,
    sector: u8,
}
