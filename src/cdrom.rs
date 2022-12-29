use std::{collections::VecDeque, io::BufReader, process::Command};

use log::{debug, warn};

use crate::addressible::{AccessWidth, Addressible};

enum ControllerStatus {
    Idle,
    ParamPush,
    CommandPush,
    Execution,
    ResponseFlush,
    ResponsePush,
    IrqDelay,
}

enum CdRomStatus {
    Idle,
    Seeking,
    Reading,
}

#[derive(Clone, Copy, Debug)]
enum CdRomIrq {
    ReadReady = 1,
    SecondOk = 2,
    FirstOk = 3,
    Error = 5,
}

type AsyncCallback = dyn Fn(&mut CdRom);

pub struct CdRom {
    index: u8,

    controller: Controller,

    disc: Option<Vec<u8>>,

    parameter_fifo: VecDeque<u8>,
    response_fifo: VecDeque<u8>,
    data_fifo: VecDeque<u8>,

    // stat
    status: CdRomStatus,
    stat_updated: bool,

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

    tasks: VecDeque<(u32, Box<AsyncCallback>)>,
}

impl CdRom {
    pub fn new(disc: Option<Vec<u8>>) -> Self {
        Self {
            index: 0,
            disc,
            controller: Controller::new(),
            parameter_fifo: VecDeque::with_capacity(16),
            response_fifo: VecDeque::with_capacity(16),
            data_fifo: VecDeque::with_capacity(934),
            status: CdRomStatus::Idle,
            stat_updated: false,
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
            tasks: VecDeque::with_capacity(16),
        }
    }

    pub fn load<T: Addressible>(&mut self, offset: u32) -> T {
        let r = match offset {
            0 => self.status() as u32,
            1 => self.response_fifo() as u32,
            2 => match T::width() {
                AccessWidth::Word => self.data_fifo_word(),
                AccessWidth::Halfword => self.data_fifo_halfword() as u32,
                AccessWidth::Byte => self.data_fifo() as u32,
            },
            3 => match self.index {
                0 | 2 => self.ie() as u32,
                1 | 3 => self.irq() as u32,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };

        Addressible::from_u32(r)
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

    pub fn tick(&mut self) {
        if self.tasks.len() > 0 {
            if self.tasks[0].0 > 0 {
                self.tasks[0].0 -= 1;
            } else {
                let task = self.tasks.pop_front().unwrap();
                let f = task.1;
                f(self);
            }
        }

        self.controller.tick();
    }

    pub fn check_irq(&self) -> bool {
        let irq = self.irq & self.ie;

        irq != 0
    }

    fn status(&self) -> u8 {
        let mut result = 0;

        // busy
        // data fifo not empty
        // response fifo not empty
        // param fifo not full
        // param fifo empty
        // index (2bit)
        result |= self.index & 0b11;
        result |= (self.parameter_fifo.is_empty() as u8) << 3;
        result |= ((self.parameter_fifo.len() < 16) as u8) << 4;
        result |= (!self.response_fifo.is_empty() as u8) << 5;
        result |= (self.read_active as u8) << 6;
        result |= (self.busy() as u8) << 7;

        debug!("CD-ROM status read {:02x}", result);

        result
    }

    fn busy(&self) -> bool {
        match self.controller.status {
            ControllerStatus::ParamPush
            | ControllerStatus::CommandPush
            | ControllerStatus::Execution
            | ControllerStatus::ResponseFlush
            | ControllerStatus::ResponsePush
            | ControllerStatus::IrqDelay => true,
            ControllerStatus::Idle => false,
        }
    }

    fn set_index(&mut self, val: u8) {
        debug!("CD-ROM set index {}", val);
        self.index = val & 0b11;
    }

    fn ie(&self) -> u8 {
        debug!("CD-ROM read ie {:02x}", self.ie);

        self.ie
    }

    fn set_ie(&mut self, val: u8) {
        debug!("CD-ROM set ie {:02x}", val);

        self.ie = val;
    }

    fn irq(&self) -> u8 {
        debug!("CD-ROM read irq {:02x}", self.irq);

        self.irq
    }

    fn set_irq(&mut self, val: u8) {
        debug!("CD-ROM set irq {:02x}", val);

        self.irq &= !val;

        let ack = val & 0x1F;

        if ack != 0 {
            debug!("CD-ROM ack irq => response fifo cleared");
            self.response_fifo.clear();
        }

        if val & 0x40 != 0 {
            debug!("CD-ROM parameter fifo cleared");
            self.parameter_fifo.clear();
        }
    }

    fn raise_irq(&mut self, irq: CdRomIrq) {
        debug!("CD-ROM raise irq {:?}", irq);

        self.irq &= 0xF8;
        self.irq |= (irq as u8) & 0x7;
    }

    fn set_parameter_fifo(&mut self, val: u8) {
        debug!("CD-ROM parameter push {:02x}", val);
        self.parameter_fifo.push_back(val);
    }

    fn set_request_register(&mut self, val: u8) {
        self.read_active = val & 0x80 != 0;
    }

    fn response_fifo(&mut self) -> u8 {
        debug!(
            "CD-ROM response pop {:02x}",
            self.response_fifo.front().unwrap_or(&0)
        );
        self.response_fifo.pop_front().unwrap_or(0)
    }

    fn data_at(&self, offset: u16) -> u8 {
        let base = self.current_position.into_addr(self.raw_sector) as usize;
        let disc = self.disc.as_ref().unwrap();

        disc[base + offset as usize]
    }

    fn data_fifo(&mut self) -> u8 {
        if !self.read_active {
            warn!("inactive data fifo access")
        }

        let val = self.data_at(self.read_index);

        debug!("CD-ROM data pop {:02x}", val);

        self.read_index += 1;

        val
    }

    fn data_fifo_halfword(&mut self) -> u16 {
        if !self.read_active {
            warn!("inactive data fifo access")
        }

        let lower = self.data_at(self.read_index) as u16;
        let higher = self.data_at(self.read_index + 1) as u16;
        let val = (higher << 8) | lower;

        debug!("CD-ROM data pop {:04x}", val);

        self.read_index += 2;

        val
    }

    fn data_fifo_word(&mut self) -> u32 {
        if !self.read_active {
            warn!("inactive data fifo access")
        }

        let lowest = self.data_at(self.read_index) as u32;
        let lower = self.data_at(self.read_index + 1) as u32;
        let higher = self.data_at(self.read_index + 2) as u32;
        let highest = self.data_at(self.read_index + 3) as u32;
        let val = (highest << 24) | (higher << 16) | (lower << 8) | lowest;

        debug!("CD-ROM data pop {:08x}", val);

        self.read_index += 4;

        val
    }

    fn stat(&mut self, update: bool) -> u8 {
        let stat_updated = self.stat_updated;

        if update {
            self.stat_updated = true;
        }

        // motor onの分+2してる
        if self.disc == None || !stat_updated {
            0x12 // shell opened
        } else {
            match self.status {
                CdRomStatus::Idle => 0x02,
                CdRomStatus::Seeking => 0x42,
                CdRomStatus::Reading => 0x22,
            }
        }
    }

    fn command(&mut self, val: u8) {
        match val {
            0x01 => self.get_stat(),
            0x02 => self.set_loc(),
            0x06 => self.read_n(),
            0x09 => self.pause(),
            0x0A => self.init(),
            0x0E => self.set_mode(),
            0x15 => self.seek_l(),
            0x19 => self.test(),
            0x1A => self.get_id(),
            0x1B => self.read_s(),
            0x1E => self.read_toc(),
            _ => {
                warn!("unsupported CD-ROM command {:02x}", val);
                return;
            }
        }

        self.parameter_fifo.clear();
        debug!(
            "CD-ROM command end param fifo cleared {}",
            self.parameter_fifo.is_empty()
        );
    }

    fn get_stat(&mut self) {
        debug!("CD-ROM command getStat");
        self.tasks.push_back((
            50000,
            Box::new(|this| {
                let stat = this.stat(true);
                this.response_fifo.push_back(stat);
                this.raise_irq(CdRomIrq::FirstOk);
            }),
        ));
    }

    fn init(&mut self) {
        debug!("CD-ROM command init");

        self.tasks.push_back((
            50000,
            Box::new(|this| {
                let stat = this.stat(false);
                this.response_fifo.push_back(stat);
                this.raise_irq(CdRomIrq::FirstOk);
            }),
        ));

        self.tasks.push_back((
            900000,
            Box::new(|this| {
                this.double_speed = false;
                this.raw_sector = false;
                let stat = this.stat(false);
                this.response_fifo.push_back(stat);
                this.raise_irq(CdRomIrq::SecondOk);
            }),
        ));
    }

    fn set_mode(&mut self) {
        let mode = self.parameter_fifo[0];

        debug!("CD-ROM command setMode {:02x}", mode);

        self.tasks.push_back((
            50000,
            Box::new(move |this| {
                this.double_speed = mode & 0x80 != 0;
                this.raw_sector = mode & 0x20 != 0;

                let stat = this.stat(false);
                this.response_fifo.push_back(stat);
                this.raise_irq(CdRomIrq::FirstOk);
            }),
        ));
    }

    fn set_loc(&mut self) {
        let addr = Mss {
            min: self.parameter_fifo[0],
            sec: self.parameter_fifo[1],
            sector: self.parameter_fifo[2],
        };

        debug!("CD-ROM command setLoc {:?}", addr);

        self.tasks.push_back((
            50000,
            Box::new(move |this| {
                this.seek_position = Some(addr);

                let stat = this.stat(false);
                this.response_fifo.push_back(stat);
                this.raise_irq(CdRomIrq::FirstOk);
            }),
        ));
    }

    fn read_n(&mut self) {
        debug!("CD-ROM command readN");

        self.tasks.push_back((
            50000,
            Box::new(|this| {
                let stat = this.stat(false);
                this.response_fifo.push_back(stat);
                this.raise_irq(CdRomIrq::FirstOk);
            }),
        ));

        self.tasks.push_back((
            50000,
            Box::new(|this| {
                this.status = CdRomStatus::Reading;

                let stat = this.stat(false);
                this.response_fifo.push_back(stat);
                this.raise_irq(CdRomIrq::ReadReady);
            }),
        ));
    }

    fn pause(&mut self) {
        debug!("CD-ROM command pause");

        self.tasks.push_back((
            50000,
            Box::new(|this| {
                let stat = this.stat(false);
                this.response_fifo.push_back(stat);
                this.raise_irq(CdRomIrq::FirstOk);
            }),
        ));

        self.tasks.push_back((
            50000,
            Box::new(|this| {
                this.status = CdRomStatus::Idle;

                let stat = this.stat(false);
                this.response_fifo.push_back(stat);
                this.raise_irq(CdRomIrq::SecondOk);
            }),
        ));
    }

    fn read_s(&mut self) {
        self.read_n()
    }

    fn read_toc(&mut self) {
        debug!("CD-ROM command readToc");

        let stat = self.stat(false);
        self.response_fifo.push_back(stat);
        self.raise_irq(CdRomIrq::FirstOk);

        self.tasks.push_back((
            50000,
            Box::new(|this| {
                let stat = this.stat(false);
                this.response_fifo.push_back(stat);
                this.raise_irq(CdRomIrq::SecondOk);
            }),
        ));
    }

    fn seek_l(&mut self) {
        debug!("CD-ROM command seekL");

        if let Some(position) = self.seek_position {
            self.current_position = position;
        }

        self.tasks.push_back((
            50000,
            Box::new(|this| {
                this.status = CdRomStatus::Seeking;

                let stat = this.stat(false);
                this.response_fifo.push_back(stat);
                this.raise_irq(CdRomIrq::FirstOk);
            }),
        ));

        self.tasks.push_back((
            50000,
            Box::new(|this| {
                this.status = CdRomStatus::Idle;
                this.read_index = 0;

                let stat = this.stat(false);
                this.response_fifo.push_back(stat);
                this.raise_irq(CdRomIrq::SecondOk);
            }),
        ));
    }

    fn test(&mut self) {
        debug!("CD-ROM command test 0x{:02x}", self.parameter_fifo[0]);

        match self.parameter_fifo.pop_front() {
            Some(0x20) => self.test_version(),
            Some(n) => warn!("unsupported CD-ROM test func {:02x}", n),
            _ => warn!("CD-ROM test func missing params"),
        }
    }

    fn test_version(&mut self) {
        self.tasks.push_back((
            50000,
            Box::new(|this| {
                this.response_fifo.push_back(0x96);
                this.response_fifo.push_back(0x09);
                this.response_fifo.push_back(0x12);
                this.response_fifo.push_back(0xC2);
                this.raise_irq(CdRomIrq::FirstOk);
            }),
        ));
    }

    fn get_id(&mut self) {
        debug!("CD-ROM command getId");

        self.tasks.push_back((
            50000,
            Box::new(|this| {
                let stat = this.stat(false);
                this.response_fifo.push_back(stat);
                this.raise_irq(CdRomIrq::FirstOk);
            }),
        ));

        if self.disc == None {
            self.tasks.push_back((
                50000,
                Box::new(|this| {
                    this.response_fifo.push_back(0x08);
                    this.response_fifo.push_back(0x40);

                    this.response_fifo.push_back(0x00);
                    this.response_fifo.push_back(0x00);

                    this.response_fifo.push_back(0x00);
                    this.response_fifo.push_back(0x00);
                    this.response_fifo.push_back(0x00);
                    this.response_fifo.push_back(0x00);
                    this.raise_irq(CdRomIrq::Error);
                }),
            ));
        } else {
            self.tasks.push_back((
                50000,
                Box::new(|this| {
                    this.response_fifo.push_back(0x02);
                    this.response_fifo.push_back(0x00);

                    this.response_fifo.push_back(0x20);
                    this.response_fifo.push_back(0x00);

                    // SCEI
                    this.response_fifo.push_back(0x53);
                    this.response_fifo.push_back(0x43);
                    this.response_fifo.push_back(0x45);
                    this.response_fifo.push_back(0x49);
                    this.raise_irq(CdRomIrq::SecondOk);
                }),
            ));
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Mss {
    min: u8,
    sec: u8,
    sector: u8,
}

impl Mss {
    fn into_addr(&self, raw: bool) -> u32 {
        (self.sector as u32) * if raw { 924 } else { 800 }
    }
}

struct Controller {
    command: Option<u32>,
    status: ControllerStatus,
    stalls: u32,
    param: VecDeque<u8>,
    response: VecDeque<u8>,
    irq: Option<CdRomIrq>,
    task: Option<(u16, Box<AsyncCallback>)>,
}

impl Controller {
    fn new() -> Self {
        Self {
            command: None,
            status: ControllerStatus::Idle,
            stalls: 0,
            param: VecDeque::new(),
            response: VecDeque::new(),
            irq: None,
            task: None,
        }
    }

    fn tick(&mut self) {
        if self.stalls > 0 {
            self.stalls -= 1;
            return;
        }

        //if let Some((delay, handler)) = self.task {
        //    if delay == 0 {
        //        handler()
        //    }
        //}
    }
}
