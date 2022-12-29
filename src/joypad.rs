use std::collections::VecDeque;

use log::debug;

use crate::addressible::Addressible;

pub struct Joypad {
    select: bool,
    target: bool,
    tx_enabled: bool,
    tx: VecDeque<u8>,
    rx_enabled: bool,
    rx: VecDeque<u8>,
    ack: bool,
    acked: bool,
    irq: bool,
    baud_timer: u16,
    baud_rate: u16,
    mode: u16,
}

impl Joypad {
    pub fn new() -> Self {
        Joypad {
            select: false,
            target: false,
            tx_enabled: true,
            tx: VecDeque::new(),
            rx_enabled: true,
            rx: VecDeque::new(),
            ack: false,
            acked: false,
            irq: false,
            baud_timer: 0,
            baud_rate: 0,
            mode: 0,
        }
    }

    pub fn tick(&mut self) {
        if self.tx_enabled && !self.tx.is_empty() {
            let cmd = self.tx.pop_front().unwrap();
            self.command(cmd);
        }
    }

    pub fn load<T: Addressible>(&mut self, offset: u32) -> T {
        match offset {
            0 => {
                let res: T = Addressible::from_u32(self.rx.pop_front().unwrap_or(0) as u32);

                debug!("JOYPAD RX POP {:02x}", res.as_u32() as u8);

                res
            }
            4 => Addressible::from_u32(self.stat().as_u32()),
            8 => Addressible::from_u32(self.mode().as_u32()),
            10 => Addressible::from_u32(self.ctrl().as_u32()),
            14 => {
                let res: T = Addressible::from_u32(self.baud_rate as u32);

                debug!("JOYPAD BAUD RATE {:02x}", res.as_u32() as u8);

                res
            }
            _ => panic!("unhandled Joypad load offset: {:04x}", offset,),
        }
    }

    pub fn store<T: Addressible>(&mut self, offset: u32, val: T) {
        match offset {
            0 => {
                debug!("JOYPAD TX {:02x}", val.as_u32() as u8);
                self.tx.push_back(val.as_u32() as u8);
            }
            8 => self.set_mode(val.unwrap_u16()),
            10 => self.set_ctrl(val.unwrap_u16()),
            14 => {
                debug!("JOYPAD SET BAUD RATE {:04x}", val.unwrap_u16());
                self.baud_rate = val.unwrap_u16();
            }
            _ => panic!(
                "unhandled Joypad store offset: {:04x}, val: {:04x}",
                offset,
                val.as_u32()
            ),
        }
    }

    fn command(&mut self, command: u8) {
        match command {
            0x01 => self.command_access(),
            _ => debug!("JOYPAD unhandled COMMAND {:02x}", command),
        }
    }

    fn command_access(&mut self) {
        self.rx.push_back(0);
    }

    fn stat(&self) -> u32 {
        let mut res = 0;

        res |= self.tx.is_empty() as u32;
        res |= (!self.rx.is_empty() as u32) << 1;
        // parity errorは無し
        res |= (self.ack as u32) << 7;
        res |= (self.irq as u32) << 9;
        res |= (self.baud_timer as u32) << 11;

        debug!("JOYPAD stat {:02x}", res);

        res
    }

    fn mode(&self) -> u16 {
        self.mode
    }

    fn set_mode(&mut self, val: u16) {
        debug!("JOYPAD SET MODE {:04x}", val);
        // TODO: ちゃんとやるかもしれない
        self.mode = val;
    }

    fn ctrl(&self) -> u16 {
        let mut res = 0u16;

        res |= self.tx_enabled as u16;
        res |= (self.select as u16) << 1;
        res |= (self.rx_enabled as u16) << 2;
        res |= (self.acked as u16) << 12;
        res |= (self.target as u16) << 13;

        debug!("JOYPAD CTRL {:04x}", res);

        res
    }

    fn set_ctrl(&mut self, val: u16) {
        debug!("JOYPAD SET CTRL {:04x}", val);

        self.tx_enabled = val & 1 > 0;
        self.select = (val >> 1) & 1 > 0;
        self.rx_enabled = (val >> 2) & 1 > 0;

        // ack
        if (val >> 4) & 1 > 0 {
            self.irq = false;
        }

        // reset
        if (val >> 6) & 1 > 0 {
            // most joy registers set to zero
        }

        if self.select {
            self.target = (val << 13) & 1 > 0;
        }
    }
}
