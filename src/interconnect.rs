use log::{debug, trace, warn};

use crate::{
    addressible::{AccessWidth, Addressible},
    bios::Bios,
    cdrom::CdRom,
    dma::{Direction, Dma, Port, Step, Sync},
    gpu::gpu::Gpu,
    interrupts::{Interrupts, Irq},
    joypad::Joypad,
    ram::Ram,
    scratchpad::ScratchPad,
    timer::Timer,
};

pub struct Interconnect {
    pub bios: Bios,
    scratchpad: ScratchPad,
    ram: Ram,
    dma: Dma,
    gpu: Gpu,
    cdrom: CdRom,
    joypad: Joypad,
    timers: [Timer; 3],
    pub interrupts: Interrupts,
}

impl Interconnect {
    pub fn new(bios: Bios, gpu: Gpu, rom: Option<Vec<u8>>) -> Interconnect {
        Interconnect {
            bios,
            scratchpad: ScratchPad::new(),
            ram: Ram::new(),
            dma: Dma::new(),
            gpu,
            cdrom: CdRom::new(rom),
            joypad: Joypad::new(),
            timers: [Timer::new(0), Timer::new(1), Timer::new(2)],
            interrupts: Interrupts::new(),
        }
    }

    pub fn load<T: Addressible>(&mut self, abs_addr: u32) -> T {
        let addr = map::mask_region(abs_addr);

        trace!(
            "load{:?} addr: {:08x} -> {:08x}",
            T::width(),
            abs_addr,
            addr
        );

        if let Some(offset) = map::EXPANSION_1.contains(addr) {
            warn!("EXPANSION 1 read {}", offset);
            return Addressible::from_u32(0);
        }

        if let Some(offset) = map::RAM.contains(addr) {
            return self.ram.load(offset);
        }

        if let Some(offset) = map::SCRATCHPAD.contains(addr) {
            return self.scratchpad.load(offset);
        }

        if let Some(offset) = map::BIOS.contains(addr) {
            return self.bios.load(offset);
        }

        if let Some(offset) = map::MEM_CONTROL.contains(addr) {
            match offset {
                0 => {
                    return Addressible::from_u32(0x1f000000);
                }
                4 => {
                    return Addressible::from_u32(0x1f800200);
                }
                _ => warn!("Unhandled read to MEM_CONTROL register"),
            }
        }

        if let Some(_) = map::RAM_SIZE.contains(addr) {
            return Addressible::from_u32(0);
        }

        if let Some(_) = map::CACHE_SIZE.contains(addr) {
            return Addressible::from_u32(0);
        }

        if let Some(offset) = map::IRQ_CONTROL.contains(addr) {
            return self.interrupts.load(offset);
        }

        if let Some(offset) = map::DMA.contains(addr) {
            return self.dma_reg(offset);
        }

        if let Some(offset) = map::GPU.contains(addr) {
            return self.gpu.load(offset);
        }

        if let Some(offset) = map::CDROM.contains(addr) {
            return self.cdrom.load(offset);
        }

        if let Some(offset) = map::TIMER_0.contains(addr) {
            return self.timers[0].load(offset);
        }

        if let Some(offset) = map::TIMER_1.contains(addr) {
            return self.timers[1].load(offset);
        }

        if let Some(offset) = map::TIMER_2.contains(addr) {
            return self.timers[2].load(offset);
        }

        if let Some(offset) = map::JOYPAD.contains(addr) {
            return self.joypad.load(offset);
        }

        if let Some(offset) = map::SIO.contains(addr) {
            warn!("SIO read {}", offset);
            return Addressible::from_u32(0);
        }

        if let Some(offset) = map::EXPANSION_2.contains(addr) {
            warn!("EXPANSION 2 read {}", offset);
            return Addressible::from_u32(0);
        }

        if let Some(offset) = map::EXPANSION_3.contains(addr) {
            warn!("EXPANSION 3 read {}", offset);
            return Addressible::from_u32(0);
        }

        warn!("unhandled load{:?} at address {:08x}", T::width(), abs_addr);
        return Addressible::from_u32(0);
    }

    pub fn store<T: Addressible>(&mut self, abs_addr: u32, val: T) {
        let addr = map::mask_region(abs_addr);

        trace!(
            "store{:?} addr: {:08x} -> {:08x} = {:08x}",
            T::width(),
            abs_addr,
            addr,
            val.as_u32(),
        );

        if let Some(offset) = map::RAM.contains(addr) {
            return self.ram.store(offset, val);
        }

        if let Some(offset) = map::SCRATCHPAD.contains(addr) {
            return self.scratchpad.store(offset, val);
        }

        if let Some(_) = map::BIOS.contains(addr) {
            panic!("Invalid write to BIOS addr {:08x}", addr);
        }

        if let Some(offset) = map::MEM_CONTROL.contains(addr) {
            match offset {
                0 => {
                    if val.as_u32() != 0x1f000000 {
                        panic!("Bad expansion 1 base address: 0x{:08x}", val.as_u32());
                    }
                }
                4 => {
                    if val.as_u32() != 0x1f802000 {
                        panic!("Bad expansion 2 base address: 0x{:08x}", val.as_u32());
                    }
                }
                20 => {
                    warn!("MEM_CONTROL write {:08X}", val.as_u32())
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
            return self.interrupts.store(offset, val);
        }

        if let Some(offset) = map::DMA.contains(addr) {
            return self.set_dma_reg(offset, val);
        }

        if let Some(offset) = map::GPU.contains(addr) {
            return self.gpu.store(offset, val);
        }

        if let Some(offset) = map::CDROM.contains(addr) {
            return self.cdrom.store(offset, val);
        }

        if let Some(offset) = map::SPU.contains(addr) {
            warn!("SPU write {} {}", offset, val.as_u32());
            return;
        }

        if let Some(offset) = map::JOYPAD.contains(addr) {
            return self.joypad.store(offset, val);
        }

        if let Some(offset) = map::SIO.contains(addr) {
            warn!("SIO write {}", offset);
            return;
        }

        if let Some(offset) = map::TIMER_0.contains(addr) {
            return self.timers[0].store(offset, val);
        }

        if let Some(offset) = map::TIMER_1.contains(addr) {
            return self.timers[1].store(offset, val);
        }

        if let Some(offset) = map::TIMER_2.contains(addr) {
            return self.timers[2].store(offset, val);
        }

        if let Some(offset) = map::EXPANSION_2.contains(addr) {
            match offset {
                0x23 => debug!("{:?}", (val.as_u32() as u8) as char),
                0x2B => debug!("{:?}", (val.as_u32() as u8) as char),
                0x41 | 0x42 => debug!("BOOT STATUS: {:02x}", val.as_u32() as u8),
                0x70 => debug!("BOOT STATUS2: {:02x}", val.as_u32() as u8),
                _ => warn!(
                    "EXPANSION 2 write {:02x} = {:02x}",
                    offset,
                    val.as_u32() as u8
                ),
            }
            return;
        }

        if let Some(offset) = map::EXPANSION_3.contains(addr) {
            warn!("EXPANSION 3 write {}", offset);
            return;
        }

        warn!(
            "unhandled store{:?} into address {:08x}",
            T::width(),
            abs_addr
        );
    }

    pub fn tick(&mut self) {
        self.cdrom.tick();
        self.gpu.tick();
        self.joypad.tick();

        self.timers[0].tick(self.gpu.hblank, self.gpu.vblank, self.gpu.dotclock);
        self.timers[1].tick(self.gpu.hblank, self.gpu.vblank, self.gpu.dotclock);
        self.timers[2].tick(self.gpu.hblank, self.gpu.vblank, self.gpu.dotclock);

        self.interrupts.set(Irq::VBlank, self.gpu.vblank);
        self.interrupts.set(Irq::Gpu, self.gpu.interrupt);
        self.interrupts.set(Irq::CdRom, self.cdrom.check_irq());
        self.interrupts.set(Irq::Dma, self.dma.check_irq());
        self.interrupts.set(Irq::Tmr0, !self.timers[0].n_irq);
        self.interrupts.set(Irq::Tmr1, !self.timers[1].n_irq);
        self.interrupts.set(Irq::Tmr2, !self.timers[2].n_irq);

        self.interrupts.tick();
    }

    fn dma_reg<T: Addressible>(&self, offset: u32) -> T {
        if T::width() != AccessWidth::Word {
            panic!("Unhandled {:?} DMA load", T::width());
        }

        let major = (offset & 0x70) >> 4;
        let minor = offset & 0x0F;

        let res = match major {
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
        };

        Addressible::from_u32(res)
    }

    fn set_dma_reg<T: Addressible>(&mut self, offset: u32, val: T) {
        if T::width() != AccessWidth::Word {
            panic!("Unhandled {:?} DMA store", T::width());
        }

        let val = val.as_u32();

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
                    let src_word = self.ram.load(cur_addr);

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
                        Port::Gpu => {
                            //warn!("Unhandled DMA source port GPU");
                            0
                        }
                        Port::CdRom => self.cdrom.load(2),
                        _ => panic!("Unhandled DMA source port {}", port as u8),
                    };

                    self.ram.store(cur_addr, src_word);
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
            let header: u32 = self.ram.load(addr);

            let mut remsz = header >> 24;

            while remsz > 0 {
                addr = (addr + 4) & 0x1FFFFC;

                let command = self.ram.load(addr);

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
    pub const TIMER_0: Range = Range(0x1F801100, 12);
    pub const TIMER_1: Range = Range(0x1F801110, 12);
    pub const TIMER_2: Range = Range(0x1F801120, 12);
    pub const CDROM: Range = Range(0x1F801800, 4);
    pub const GPU: Range = Range(0x1F801810, 16);
    pub const SPU: Range = Range(0x1F801C00, 640);
    pub const EXPANSION_2: Range = Range(0x1F802000, 66);
    pub const EXPANSION_3: Range = Range(0x1FA00000, 2048 * 1024);
    pub const BIOS: Range = Range(0x1FC00000, 512 * 1024);
    pub const CACHE_SIZE: Range = Range(0xFFFE0130, 4);
}
