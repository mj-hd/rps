use log::{trace, warn};

use crate::{
    bios::Bios,
    dma::{Direction, Dma, Port, Step, Sync},
    gpu::gpu::Gpu,
    ram::Ram,
    scratchpad::ScratchPad,
};

pub struct Interconnect {
    bios: Bios,
    scratchpad: ScratchPad,
    ram: Ram,
    dma: Dma,
    gpu: Gpu,
}

impl Interconnect {
    pub fn new(bios: Bios, gpu: Gpu) -> Interconnect {
        Interconnect {
            bios,
            scratchpad: ScratchPad::new(),
            ram: Ram::new(),
            dma: Dma::new(),
            gpu,
        }
    }

    pub fn load32(&self, abs_addr: u32) -> u32 {
        trace!("load32: 0x{:08x}", abs_addr);

        if abs_addr % 4 != 0 {
            panic!("Unaligned load32 address: {:08x}", abs_addr);
        }

        let addr = map::mask_region(abs_addr);

        trace!("load32 addr: {:08x} -> {:08x}", abs_addr, addr);

        if let Some(offset) = map::EXPANSION_1.contains(addr) {
            warn!("EXPANSION 1 read {}", offset);
            return 0;
        }

        if let Some(offset) = map::RAM.contains(addr) {
            return self.ram.load32(offset);
        }

        if let Some(offset) = map::SCRATCHPAD.contains(addr) {
            return self.scratchpad.load32(offset);
        }

        if let Some(offset) = map::BIOS.contains(addr) {
            return self.bios.load32(offset);
        }

        if let Some(offset) = map::MEM_CONTROL.contains(addr) {
            match offset {
                0 => {
                    return 0x1f000000;
                }
                4 => {
                    return 0x1f800200;
                }
                _ => warn!("Unhandled write to MEM_CONTROL register"),
            }
        }

        if let Some(_) = map::RAM_SIZE.contains(addr) {
            return 0;
        }

        if let Some(_) = map::CACHE_SIZE.contains(addr) {
            return 0;
        }

        if let Some(offset) = map::IRQ_CONTROL.contains(addr) {
            warn!("IRQ control read {:x}", offset);
            return 0;
        }

        if let Some(offset) = map::DMA.contains(addr) {
            return self.dma_reg(offset);
        }

        if let Some(offset) = map::GPU.contains(addr) {
            return match offset {
                0 => self.gpu.read(),
                4 => self.gpu.status(),
                _ => 0,
            };
        }

        if let Some(offset) = map::CDROM.contains(addr) {
            warn!("CDROM read {}", offset);
            return 0;
        }

        if let Some(offset) = map::TIMERS.contains(addr) {
            warn!("TIMERS read {}", offset);
            return 0;
        }

        if let Some(offset) = map::JOYPAD.contains(addr) {
            warn!("JOYPAD read {}", offset);
            return 0;
        }

        if let Some(offset) = map::SIO.contains(addr) {
            warn!("SIO read {}", offset);
            return 0;
        }

        if let Some(offset) = map::EXPANSION_3.contains(addr) {
            warn!("EXPANSION 3 read {}", offset);
            return 0;
        }

        warn!("unhandled load32 at address {:08x}", abs_addr);
        return 0;
    }

    pub fn load16(&self, abs_addr: u32) -> u16 {
        trace!("load16: 0x{:08x}", abs_addr);

        if abs_addr % 2 != 0 {
            panic!("Unaligned load16 address: {:08x}", abs_addr);
        }

        let addr = map::mask_region(abs_addr);

        if let Some(offset) = map::RAM.contains(addr) {
            return self.ram.load16(offset);
        }

        if let Some(offset) = map::SCRATCHPAD.contains(addr) {
            return self.scratchpad.load16(offset);
        }

        if let Some(offset) = map::SPU.contains(addr) {
            warn!("Unhandled load to SPU register {}", offset);
            return 0;
        }

        if let Some(offset) = map::IRQ_CONTROL.contains(addr) {
            warn!("IRQ Control read {}", offset);
            return 0;
        }
        if let Some(offset) = map::JOYPAD.contains(addr) {
            warn!("JOYPAD read {}", offset);
            return 0;
        }

        if let Some(offset) = map::SIO.contains(addr) {
            warn!("SIO read {}", offset);
            return 0;
        }

        if let Some(offset) = map::TIMERS.contains(addr) {
            warn!("Unhandled read to timer register {}", offset);
            return 0;
        }

        warn!("unhandled load16 into address: {:08x}", abs_addr);
        return 0;
    }

    pub fn load8(&self, abs_addr: u32) -> u8 {
        trace!("load8: 0x{:08x}", abs_addr);

        let addr = map::mask_region(abs_addr);

        if let Some(offset) = map::RAM.contains(addr) {
            return self.ram.load8(offset);
        }

        if let Some(offset) = map::SCRATCHPAD.contains(addr) {
            return self.scratchpad.load8(offset);
        }

        if let Some(offset) = map::BIOS.contains(addr) {
            return self.bios.load8(offset);
        }

        if let Some(offset) = map::CDROM.contains(addr) {
            warn!("CDROM read {}", offset);
            return 0;
        }

        if let Some(_) = map::EXPANSION_1.contains(addr) {
            return 0xFF;
        }

        if let Some(offset) = map::JOYPAD.contains(addr) {
            warn!("JOYPAD read {}", offset);
            return 0;
        }

        if let Some(offset) = map::SIO.contains(addr) {
            warn!("SIO read {}", offset);
            return 0;
        }

        if let Some(offset) = map::EXPANSION_2.contains(addr) {
            warn!("Unhandled write to expansion 2 register {}", offset);
            return 0;
        }

        warn!("unhandled load8 into address: {:08x}", abs_addr);
        return 0;
    }

    pub fn store32(&mut self, abs_addr: u32, val: u32) {
        trace!("store32: 0x{:08x} => 0x{:08x}", abs_addr, val);

        if abs_addr % 4 != 0 {
            panic!("Unaligned store32 address: {:08x}", abs_addr);
        }

        let addr = map::mask_region(abs_addr);

        if let Some(offset) = map::RAM.contains(addr) {
            return self.ram.store32(offset, val);
        }

        if let Some(offset) = map::SCRATCHPAD.contains(addr) {
            return self.scratchpad.store32(offset, val);
        }

        if let Some(_) = map::BIOS.contains(addr) {
            panic!("Invalid write to BIOS addr {:08x}", addr);
        }

        if let Some(offset) = map::MEM_CONTROL.contains(addr) {
            match offset {
                0 => {
                    if val != 0x1f000000 {
                        panic!("Bad expansion 1 base address: 0x{:08x}", val);
                    }
                }
                4 => {
                    if val != 0x1f802000 {
                        panic!("Bad expansion 2 base address: 0x{:08x}", val);
                    }
                }
                _ => warn!("Unhandled write to MEM_CONTROL register"),
            }
            return;
        }

        if let Some(_) = map::RAM_SIZE.contains(addr) {
            return;
        }

        if let Some(_) = map::CACHE_SIZE.contains(addr) {
            return;
        }

        if let Some(offset) = map::IRQ_CONTROL.contains(addr) {
            warn!("IRQ control: {:x} <- {:08x}", offset, val);
            return;
        }

        if let Some(offset) = map::DMA.contains(addr) {
            return self.set_dma_reg(offset, val);
        }

        if let Some(offset) = map::GPU.contains(addr) {
            match offset {
                0 => self.gpu.gp0(val),
                4 => self.gpu.gp1(val),
                _ => panic!("GPU write {} {:08x}", offset, val),
            }
            return;
        }

        if let Some(offset) = map::CDROM.contains(addr) {
            warn!("CDROM write {} {}", offset, val);
            return;
        }

        if let Some(offset) = map::SPU.contains(addr) {
            warn!("SPU write {} {}", offset, val);
            return;
        }

        if let Some(offset) = map::JOYPAD.contains(addr) {
            warn!("JOYPAD write {} {}", offset, val);
            return;
        }

        if let Some(offset) = map::SIO.contains(addr) {
            warn!("SIO write {}", offset);
            return;
        }

        if let Some(offset) = map::TIMERS.contains(addr) {
            warn!("TIMER write {} {}", offset, val);
            return;
        }

        warn!("unhandled store32 into address {:08x}", abs_addr);
    }

    pub fn store16(&mut self, abs_addr: u32, val: u16) {
        trace!("store16: 0x{:08x} => 0x{:04x}", abs_addr, val);

        if abs_addr % 2 != 0 {
            panic!("Unaligned store16 address: {:08x}", abs_addr);
        }

        let addr = map::mask_region(abs_addr);

        if let Some(offset) = map::RAM.contains(addr) {
            return self.ram.store16(offset, val);
        }

        if let Some(offset) = map::SCRATCHPAD.contains(addr) {
            return self.scratchpad.store16(offset, val);
        }

        if let Some(offset) = map::IRQ_CONTROL.contains(addr) {
            warn!("IRQ control write {:x} {:04x}", offset, val);
            return;
        }

        if let Some(offset) = map::SPU.contains(addr) {
            warn!("Unhandled write to SPU register {:08x}", offset);
            return;
        }
        if let Some(offset) = map::JOYPAD.contains(addr) {
            warn!("JOYPAD write {} {}", offset, val);
            return;
        }

        if let Some(offset) = map::SIO.contains(addr) {
            warn!("SIO write {}", offset);
            return;
        }

        if let Some(offset) = map::TIMERS.contains(addr) {
            warn!("Unhandled write to timer register {:x}", offset);
            return;
        }

        warn!("unhandled store16 into address: {:08x}", abs_addr);
    }

    pub fn store8(&mut self, abs_addr: u32, val: u8) {
        trace!("store8: 0x{:08x} => 0x{:02x}", abs_addr, val);

        let addr = map::mask_region(abs_addr);

        if let Some(offset) = map::RAM.contains(addr) {
            return self.ram.store8(offset, val);
        }

        if let Some(offset) = map::SCRATCHPAD.contains(addr) {
            return self.scratchpad.store8(offset, val);
        }

        if let Some(offset) = map::CDROM.contains(addr) {
            warn!("CDROM write {} {}", offset, val);
            return;
        }

        if let Some(offset) = map::EXPANSION_1.contains(addr) {
            warn!("Unhandled write to expansion 1 register {:08x}", offset);
            return;
        }

        if let Some(offset) = map::EXPANSION_2.contains(addr) {
            warn!("Unhandled write to expansion 2 register {:08x}", offset);
            return;
        }

        if let Some(offset) = map::JOYPAD.contains(addr) {
            warn!("JOYPAD write {} {}", offset, val);
            return;
        }

        if let Some(offset) = map::SIO.contains(addr) {
            warn!("SIO write {}", offset);
            return;
        }

        warn!("unhandled store8 into address: {:08x}", abs_addr);
    }

    fn dma_reg(&self, offset: u32) -> u32 {
        let major = (offset & 0x70) >> 4;
        let minor = offset & 0x0F;

        match major {
            0..=6 => {
                let channel = self.dma.channel(Port::from_index(major));

                match minor {
                    8 => channel.control(),
                    _ => panic!("Unhandled DMA read at {:x}", offset),
                }
            }
            7 => match minor {
                0 => self.dma.control(),
                4 => self.dma.interrupt(),
                _ => panic!("Unhandled DMA read at {:x}", offset),
            },
            _ => panic!("unhandled DMA read {:x}", offset),
        }
    }

    fn set_dma_reg(&mut self, offset: u32, val: u32) {
        let major = (offset & 0x70) >> 4;
        let minor = offset & 0x0F;

        let active_port = match major {
            0..=6 => {
                let port = Port::from_index(major);
                let channel = self.dma.channel_mut(port);

                match minor {
                    0 => channel.set_base(val),
                    4 => channel.set_block_control(val),
                    8 => channel.set_control(val),
                    _ => panic!("Unhandled DMA write at {:x}: {:08x}", offset, val),
                }

                if channel.active() {
                    Some(port)
                } else {
                    None
                }
            }
            7 => {
                match minor {
                    0 => self.dma.set_control(val),
                    4 => self.dma.set_interrupt(val),
                    _ => panic!("Unhandled DMA write at {:x}: {:08x}", offset, val),
                };

                None
            }
            _ => panic!("unhandled DMA write {:x}", offset),
        };

        if let Some(active_port) = active_port {
            self.do_dma(active_port);
        }
    }

    fn do_dma(&mut self, port: Port) {
        match self.dma.channel(port).sync() {
            Sync::LinkedList => self.do_dma_linked_list(port),
            _ => self.do_dma_block(port),
        }
    }

    fn do_dma_block(&mut self, port: Port) {
        let channel = self.dma.channel_mut(port);

        let increment = match channel.step() {
            Step::Increment => 4i32,
            Step::Decrement => -4i32,
        };

        let mut addr = channel.base();

        let mut remsz = match channel.transfer_size() {
            Some(n) => n,
            None => panic!("Couldn't figure out DMA block transfer size"),
        };

        while remsz > 0 {
            let cur_addr = addr & 0x1FFFFC;

            match channel.direction() {
                Direction::FromRam => {
                    let src_word = self.ram.load32(cur_addr);

                    match port {
                        Port::Gpu => self.gpu.gp0(src_word),
                        _ => panic!("Unhandled DMA destination port {}", port as u8),
                    }
                }
                Direction::ToRam => {
                    let src_word = match port {
                        Port::Otc => match remsz {
                            1 => 0xFFFFFF,
                            _ => addr.wrapping_sub(4) & 0x1FFFFF,
                        },
                        _ => panic!("Unhandled DMA source port {}", port as u8),
                    };

                    self.ram.store32(cur_addr, src_word);
                }
            }

            addr = (addr as i32).wrapping_add(increment) as u32;
            remsz -= 1;
        }

        channel.done();
    }

    fn do_dma_linked_list(&mut self, port: Port) {
        let channel = self.dma.channel_mut(port);

        let mut addr = channel.base() & 0x1FFFFC;

        if channel.direction() == Direction::ToRam {
            panic!("Invalid DMA direction for linked list mode");
        }

        if port != Port::Gpu {
            panic!("Attempted linked list DMa on port {}", port as u8);
        }

        // 8bit     | 24bit
        // commands | next header addr
        loop {
            let header = self.ram.load32(addr);

            let mut remsz = header >> 24;

            while remsz > 0 {
                addr = (addr + 4) & 0x1FFFFC;

                let command = self.ram.load32(addr);

                self.gpu.gp0(command);

                remsz -= 1;
            }

            if header & 0x800000 != 0 {
                break;
            }

            addr = header & 0x1FFFFC;
        }

        channel.done();
    }
}

mod map {
    pub struct Range(u32, u32); // (start, length)

    impl Range {
        // addr: 絶対アドレス
        // 戻り値: 相対アドレス
        pub fn contains(self, addr: u32) -> Option<u32> {
            let Range(start, length) = self;

            if start <= addr && addr < start + length {
                Some(addr - start)
            } else {
                None
            }
        }
    }

    const REGION_MASK: [u32; 8] = [
        // KUSEG
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, //
        // KSEG0
        0x7FFFFFFF, //
        // KSEG1
        0x1FFFFFFF, //
        // KSEG2
        0xFFFFFFFF, 0xFFFFFFFF,
    ];

    pub fn mask_region(addr: u32) -> u32 {
        let index = (addr >> 29) as usize;

        addr & REGION_MASK[index]
    }

    pub const RAM: Range = Range(0x00000000, 2 * 1024 * 1024);
    pub const EXPANSION_1: Range = Range(0x1F000000, 256);
    pub const SCRATCHPAD: Range = Range(0x1F800000, 0x400);
    pub const MEM_CONTROL: Range = Range(0x1F801000, 36);
    pub const JOYPAD: Range = Range(0x1F801040, 16);
    pub const SIO: Range = Range(0x1F801050, 16);
    pub const RAM_SIZE: Range = Range(0x1F801060, 4);
    pub const IRQ_CONTROL: Range = Range(0x1F801070, 8);
    pub const DMA: Range = Range(0x1F801080, 0x80);
    pub const TIMERS: Range = Range(0x1F801100, 48);
    pub const CDROM: Range = Range(0x1F801800, 4);
    pub const GPU: Range = Range(0x1F801810, 16);
    pub const SPU: Range = Range(0x1F801C00, 640);
    pub const EXPANSION_2: Range = Range(0x1F802000, 66);
    pub const EXPANSION_3: Range = Range(0x1FA00000, 2048 * 1024);
    pub const BIOS: Range = Range(0x1FC00000, 512 * 1024);
    pub const CACHE_SIZE: Range = Range(0xFFFE0130, 4);
}
