use crate::{bios::Bios, ram::Ram};

pub struct Interconnect {
    bios: Bios,
    ram: Ram,
}

impl Interconnect {
    pub fn new(bios: Bios) -> Interconnect {
        Interconnect {
            bios,
            ram: Ram::new(),
        }
    }

    pub fn load32(&self, addr: u32) -> u32 {
        if addr % 4 != 0 {
            panic!("Unaligned load32 address: {:08x}", addr);
        }

        let addr = map::mask_region(addr);

        if let Some(offset) = map::RAM.contains(addr) {
            return self.ram.load32(offset);
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
                _ => println!("Unhandled write to MEM_CONTROL register"),
            }
        }

        if let Some(_) = map::RAM_SIZE.contains(addr) {
            return 0;
        }

        if let Some(_) = map::CACHE_SIZE.contains(addr) {
            return 0;
        }

        if let Some(offset) = map::IRQ_CONTROL.contains(addr) {
            println!("IRQ control read {:x}", offset);
            return 0;
        }

        if let Some(offset) = map::DMA.contains(addr) {
            println!("DMA read: {:08x}", addr);
            return 0;
        }

        if let Some(offset) = map::GPU.contains(addr) {
            println!("GPU read {}", offset);
            return 0;
        }

        if let Some(offset) = map::TIMERS.contains(addr) {
            println!("TIMERS read {}", offset);
            return 0;
        }

        panic!("unhandled load32 at address {:08x}", addr);
    }

    pub fn load16(&self, addr: u32) -> u16 {
        if addr % 2 != 0 {
            panic!("Unaligned load16 address: {:08x}", addr);
        }

        let addr = map::mask_region(addr);

        if let Some(offset) = map::RAM.contains(addr) {
            return self.ram.load16(offset);
        }

        if let Some(offset) = map::SPU.contains(addr) {
            println!("Unhandled load to SPU register {:08x}", offset);
            return 0;
        }

        if let Some(offset) = map::IRQ_CONTROL.contains(addr) {
            println!("IRQ Control read {:x}", offset);
            return 0;
        }

        if let Some(offset) = map::TIMERS.contains(addr) {
            println!("Unhandled read to timer register {:x}", offset);
            return 0;
        }

        panic!("unhandled load16 into address: {:08x}", addr);
    }

    pub fn load8(&self, addr: u32) -> u8 {
        let addr = map::mask_region(addr);

        if let Some(offset) = map::RAM.contains(addr) {
            return self.ram.load8(offset);
        }

        if let Some(offset) = map::BIOS.contains(addr) {
            return self.bios.load8(offset);
        }

        if let Some(_) = map::EXPANSION_1.contains(addr) {
            return 0xFF;
        }

        if let Some(offset) = map::EXPANSION_2.contains(addr) {
            println!("Unhandled write to expansion 2 register {:08x}", offset);
            return 0;
        }

        panic!("unhandled load8 into address: {:08x}", addr);
    }

    pub fn store32(&mut self, addr: u32, val: u32) {
        if addr % 4 != 0 {
            panic!("Unaligned store32 address: {:08x}", addr);
        }

        let addr = map::mask_region(addr);

        if let Some(offset) = map::RAM.contains(addr) {
            return self.ram.store32(offset, val);
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
                    if val != 0x1f800200 {
                        panic!("Bad expansion 2 base address: 0x{:08x}", val);
                    }
                }
                _ => println!("Unhandled write to MEM_CONTROL register"),
            }
        }

        if let Some(_) = map::RAM_SIZE.contains(addr) {
            return;
        }

        if let Some(_) = map::CACHE_SIZE.contains(addr) {
            return;
        }

        if let Some(offset) = map::IRQ_CONTROL.contains(addr) {
            println!("IRQ control: {:x} <- {:08x}", offset, val);
            return;
        }

        if let Some(_) = map::DMA.contains(addr) {
            println!("DMA write: {:08x} {:08x}", addr, val);
            return;
        }

        if let Some(offset) = map::GPU.contains(addr) {
            println!("GPU write {} {}", offset, val);
            return;
        }

        if let Some(offset) = map::TIMERS.contains(addr) {
            println!("TIMER write {} {}", offset, val);
            return;
        }

        panic!("unhandled store32 into address {:08x}", addr);
    }

    pub fn store16(&mut self, addr: u32, val: u16) {
        if addr % 2 != 0 {
            panic!("Unaligned store16 address: {:08x}", addr);
        }

        let addr = map::mask_region(addr);

        if let Some(offset) = map::RAM.contains(addr) {
            return self.ram.store16(offset, val);
        }

        if let Some(offset) = map::IRQ_CONTROL.contains(addr) {
            println!("IRQ control write {:x} {:04x}", offset, val);
            return;
        }

        if let Some(offset) = map::SPU.contains(addr) {
            println!("Unhandled write to SPU register {:08x}", offset);
            return;
        }

        if let Some(offset) = map::TIMERS.contains(addr) {
            println!("Unhandled write to timer register {:x}", offset);
            return;
        }

        panic!("unhandled store16 into address: {:08x}", addr);
    }

    pub fn store8(&mut self, addr: u32, val: u8) {
        let addr = map::mask_region(addr);

        if let Some(offset) = map::RAM.contains(addr) {
            return self.ram.store8(offset, val);
        }

        if let Some(offset) = map::EXPANSION_1.contains(addr) {
            println!("Unhandled write to expansion 1 register {:08x}", offset);
            return;
        }

        if let Some(offset) = map::EXPANSION_2.contains(addr) {
            println!("Unhandled write to expansion 2 register {:08x}", offset);
            return;
        }

        panic!("unhandled store8 into address: {:08x}", addr);
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
    pub const BIOS: Range = Range(0x1FC00000, 512 * 1024);
    pub const MEM_CONTROL: Range = Range(0x1F801000, 36);
    pub const RAM_SIZE: Range = Range(0x1F801060, 4);
    pub const CACHE_SIZE: Range = Range(0xFFFE0130, 4);
    pub const SPU: Range = Range(0x1F801C00, 640);
    pub const EXPANSION_1: Range = Range(0x1F000000, 256);
    pub const EXPANSION_2: Range = Range(0x1F802000, 66);
    pub const IRQ_CONTROL: Range = Range(0x1F801070, 8);
    pub const TIMERS: Range = Range(0x1F801100, 48);
    pub const DMA: Range = Range(0x1F801080, 0x80);
    pub const GPU: Range = Range(0x1F801810, 16);
}
