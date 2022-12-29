use std::{thread, time::Duration};

use log::{debug, info, trace, warn};

use crate::{addressible::Addressible, gte::Gte, interconnect::Interconnect};

use super::{instruction::Instruction, RegisterIndex};

pub enum RunEvent {
    IncomingData,
    Event(Event),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    DoneStep,
    Halted,
    Break,
    WatchWrite(u32),
    WatchRead(u32),
}

pub enum ExecMode {
    Continue,
    Step,
    RangeStep(u32, u32),
}

#[derive(Debug)]
enum Exception {
    Irq = 0x0,
    LoadAddressError = 0x4,
    StoreAddressError = 0x5,
    SysCall = 0x8,
    Overflow = 0xC,
    Break = 0x9,
    CoprocessorError = 0xB,
    IllegalInstruction = 0xA,
}

pub struct Cpu {
    pub pc: u32,
    next_pc: u32,
    pub regs: [u32; 32],
    out_regs: [u32; 32],
    pub inter: Interconnect,
    load: (RegisterIndex, u32),
    branch: bool,
    delay_slot: bool,
    stalls: u16,

    pub hi: u32,
    pub lo: u32,
    current_pc: u32,

    // COP0
    pub sr: u32,    // r12
    pub cause: u32, // r13
    pub epc: u32,   // r14

    // COP2(GTE)
    pub gte: Gte,

    pub exec_mode: ExecMode,
    pub breakpoints: Vec<u32>,
    pub watchpoints: Vec<u32>,
    event: Option<Event>,

    tty_buffer: String,
}

impl Cpu {
    pub fn new(inter: Interconnect) -> Cpu {
        let mut regs = [0xDEADBEEFu32; 32];

        regs[0] = 0;

        let pc = 0xbfc00000;

        Cpu {
            pc,
            next_pc: pc.wrapping_add(4),
            regs,
            out_regs: regs,
            inter,
            load: (RegisterIndex(0), 0),
            sr: 0,
            hi: 0xDEADBEEFu32,
            lo: 0xDEADBEEFu32,
            current_pc: 0,
            cause: 0,
            epc: 0,
            branch: false,
            delay_slot: false,
            gte: Gte::new(),
            exec_mode: ExecMode::Continue,
            breakpoints: vec![],
            watchpoints: vec![],
            event: None,
            tty_buffer: String::new(),
            stalls: 0,
        }
    }

    fn reg(&self, index: RegisterIndex) -> u32 {
        self.regs[index.0 as usize]
    }

    fn set_reg(&mut self, index: RegisterIndex, val: u32) {
        self.out_regs[index.0 as usize] = val;

        self.out_regs[0] = 0;
    }

    fn set_cause(&mut self, val: u32) {
        self.cause &= !0x300;
        self.cause |= val & 0x300;
    }

    pub fn run(&mut self, mut poll_incoming_data: impl FnMut() -> bool) -> RunEvent {
        match self.exec_mode {
            ExecMode::Continue => {
                let mut cycles = 0;

                loop {
                    if cycles % 1024 == 0 {
                        if poll_incoming_data() {
                            break RunEvent::IncomingData;
                        }
                    }

                    cycles += 1;

                    if let Some(event) = self.step() {
                        if event == Event::DoneStep {
                            continue;
                        }
                        break RunEvent::Event(event);
                    }
                }
            }
            ExecMode::Step => loop {
                if let Some(event) = self.step() {
                    break RunEvent::Event(event);
                }
            },
            ExecMode::RangeStep(start, end) => {
                let mut cycles = 0;
                loop {
                    if cycles % 1024 == 0 {
                        if poll_incoming_data() {
                            break RunEvent::IncomingData;
                        }
                    }

                    cycles += 1;

                    if let Some(event) = self.step() {
                        if event == Event::DoneStep {
                            continue;
                        }
                        break RunEvent::Event(event);
                    }

                    if !(start..end).contains(&self.pc) {
                        break RunEvent::Event(Event::DoneStep);
                    }
                }
            }
        }
    }

    pub fn step(&mut self) -> Option<Event> {
        if self.pc == 0xbfc00000 {
            thread::sleep(Duration::from_secs(3));
        }

        self.event = None;

        self.inter.tick();

        if self.stalls > 0 {
            self.stalls -= 1;

            return self.event;
        }

        self.current_pc = self.pc;

        if self.current_pc % 4 != 0 {
            self.exception(Exception::LoadAddressError);
            return Some(self.event.unwrap_or(Event::DoneStep));
        }

        self.stalls += 4; // TODO: cacheの考慮
        let instruction = Instruction(self.load::<u32>(self.pc));

        self.pc = self.next_pc;
        self.next_pc = self.next_pc.wrapping_add(4);

        let (reg, val) = self.load;
        self.set_reg(reg, val);

        self.load = (RegisterIndex(0), 0);
        self.delay_slot = self.branch;
        self.branch = false;

        if self.check_irq() {
            self.cause |= 1 << 10;
            self.stalls += 1;
            self.exception(Exception::Irq);
        } else {
            self.decode_and_execute(instruction);
        }

        self.regs = self.out_regs;

        if self.breakpoints.contains(&self.pc) {
            debug!("BREAK {:08x}", self.pc);
            self.event = Some(Event::Break);
            return self.event;
        }

        // if !self.breakpoints.is_empty() {
        //     debug!("PC: {:08x}, instr: {:08x}", self.current_pc, instruction);
        // }

        return Some(self.event.unwrap_or(Event::DoneStep));
    }

    pub fn pc(&self) -> u32 {
        self.pc
    }

    pub fn load<T: Addressible>(&mut self, addr: u32) -> T {
        if self.watchpoints.contains(&addr) {
            self.event = Some(Event::WatchRead(addr));
        }
        if addr == 0x1F801800 {
            debug!("CD-ROM Status read at {:08x}", self.current_pc);
        }
        self.stalls += 2;
        self.inter.load(addr)
    }

    pub fn store<T: Addressible>(&mut self, addr: u32, val: T) {
        if self.watchpoints.contains(&addr) {
            self.event = Some(Event::WatchWrite(addr));
        }
        if self.sr & 0x10000 != 0 {
            // warn!("Ignoring store while cache is isolated");
            return;
        }
        if addr == 0x1F801801 {
            debug!(
                "CD-ROM Command send: {:01x} at {:08x}",
                val.as_u32() as u8,
                self.current_pc
            );
        }
        self.inter.store(addr, val)
    }

    pub fn examine<T: Addressible>(&mut self, addr: u32) -> T {
        self.inter.load(addr)
    }

    pub fn put<T: Addressible>(&mut self, addr: u32, val: T) {
        self.inter.store(addr, val);
    }

    fn debug_string(&mut self, addr: u32) -> String {
        let mut result = String::new();
        let mut addr = addr;
        loop {
            let c = self.load::<u8>(addr);
            if c == 0x00 {
                break;
            }
            result.push(c as char);
            addr += 1;
        }

        result
    }

    fn debug_event_class(&self, class: u32) -> String {
        match class {
            0x00000000..=0x0000000F => format!("MemCard {:01x}", class & 0xF),
            0xF0000001 => "IRQ0 VBLANK".to_string(),
            0xF0000002 => "IRQ1 GPU".to_string(),
            0xF0000003 => "IRQ2 CDROM".to_string(),
            0xF0000004 => "IRQ3 DMA".to_string(),
            0xF0000005 => "IRQ4 RTC0".to_string(),
            0xF0000006 => "IRQ5/IRQ6 RTC1 (timer1/timer2)".to_string(),
            0xF0000007 => "not used (0xF0000007)".to_string(),
            0xF0000008 => "IRQ7 Controller (JoyPad/MemCard)".to_string(),
            0xF0000009 => "IRQ9 SPU".to_string(),
            0xF000000A => "IRQ10 PIO".to_string(),
            0xF000000B => "IRQ8 SIO".to_string(),
            0xF0000010 => "Exception".to_string(),
            0xF0000011 => "MemCard (0xF000011)".to_string(),
            0xF0000012 => "MemCard (0xF000012)".to_string(),
            0xF0000013 => "MemCard (0xF000013)".to_string(),
            0xF2000000 => "Root Counter 0 (Dotclock)".to_string(),
            0xF2000001 => "Root Counter 1 (Horizontal Retrace?)".to_string(),
            0xF2000002 => "Root Counter 2 (One-Eighth of System Clock)".to_string(),
            0xF2000003 => "Root Counter 3 (Vertical Retrace)".to_string(),
            0xF4000001 => "MemCard (higher level BIOS function)".to_string(),
            n => format!("Unknown ({:08x})", n),
        }
    }

    fn debug_event_spec(&self, spec: u32) -> String {
        match spec {
            0x0001 => "counter become zero".to_string(),
            0x0002 => "interrupted".to_string(),
            0x0004 => "end of I/O".to_string(),
            0x0008 => "file was closed".to_string(),
            0x0010 => "command ack".to_string(),
            0x0020 => "command completed".to_string(),
            0x0040 => "data ready".to_string(),
            0x0080 => "data end".to_string(),
            0x0100 => "time out".to_string(),
            0x0200 => "unknown command".to_string(),
            0x0400 => "end of read buffer".to_string(),
            0x0800 => "end of writer buffer".to_string(),
            0x1000 => "general interrupt".to_string(),
            0x2000 => "new device".to_string(),
            0x4000 => "system call instr".to_string(),
            0x8000 => "error happend".to_string(),
            0x8001 => "previous write error happened".to_string(),
            0x0301 => "domain error in libmath".to_string(),
            0x0302 => "range error in libmath".to_string(),
            n => format!("Unknown ({:08x})", n),
        }
    }

    fn debug_event_mode(&self, mode: u32) -> String {
        match mode {
            0x1000 => "exec callback function, and stay busy".to_string(),
            0x2000 => "Do NOT execute callback function, and mark event as ready".to_string(),
            n => format!("Unknown ({:08x})", n),
        }
    }

    fn branch(&mut self, offset: u32) {
        let offset = offset << 2;

        self.next_pc = self.pc.wrapping_add(offset);
        self.branch = true;
    }

    fn check_irq(&mut self) -> bool {
        self.sr & 1 != 0 && self.inter.interrupts.check()
    }

    fn exception(&mut self, cause: Exception) {
        debug!("exception: {:?} at {:08x}", cause, self.current_pc);
        let handler = match self.sr & (1 << 22) != 0 {
            true => 0xbfc00180,
            false => 0x80000080,
        };

        let mode = self.sr & 0x3F;
        self.sr &= !0x3F;
        self.sr |= (mode << 2) & 0x3F;

        self.cause &= !0x7C;
        self.cause = (cause as u32) << 2;

        self.epc = self.current_pc;

        if self.delay_slot {
            self.epc = self.epc.wrapping_sub(4);
            self.cause |= 1 << 31;
        }

        self.pc = handler;
        self.next_pc = self.pc.wrapping_add(4);
    }

    fn debug_bios_func(&mut self) {
        match self.current_pc {
            0x000000A0 => match self.regs[9] {
                0x00 => debug!(
                    "BIOS A FileOpen filename: {}, accessmode: {:08x}",
                    self.debug_string(self.regs[4]),
                    self.regs[5]
                ),
                0x01 => debug!(
                    "BIOS A FileSeek fd: {:08x}, offset: {:08x}, seektype: {:08x}",
                    self.regs[4], self.regs[5], self.regs[6],
                ),
                0x02 => debug!(
                    "BIOS A FileRead fd: {:08x}, dst: {:08x}, length: {:08x}",
                    self.regs[4], self.regs[5], self.regs[6],
                ),
                0x03 => debug!(
                    "BIOS A FileWrite fd: {:08x}, src: {:08x}, length: {:08x}",
                    self.regs[4], self.regs[5], self.regs[6],
                ),
                0x04 => debug!("BIOS A FileClose fd: {:08x}", self.regs[4]),
                0x05 => debug!(
                    "BIOS A FileIoctl fd: {:08x}, cmd: {:08x}, arg: {:08x}",
                    self.regs[4], self.regs[5], self.regs[6],
                ),
                0x06 => debug!("BIOS A exit code:{}", self.regs[4]),
                0x07 => debug!("BIOS A FileGetDeviceFlag fd: {:08x}", self.regs[4]),
                0x08 => debug!("BIOS A FileGetc fd: {:08x}", self.regs[4]),
                0x09 => debug!("BIOS A FilePutc fd: {:08x}", self.regs[4]),
                0x0A => debug!("BIOS A todigit char: {}", self.regs[4] as u8 as char),
                0x13 => debug!("BIOS A setjmp buf: {:08x}", self.regs[4]),
                0x17 => debug!("BIOS A strcmp..."),
                0x19 => debug!(
                    "BIOS A strcpy dst: {:08x}, src: {:08x}",
                    self.regs[4], self.regs[5]
                ),
                0x1B => debug!("BIOS A strlen src: {:08x}", self.regs[4]),
                0x25 => debug!("BIOS A toupper char: {}", self.regs[4] as u8 as char),
                0x28 => debug!(
                    "BIOS A bzero dst: {:08x}, len: {:08x}",
                    self.regs[4], self.regs[5]
                ),
                0x2A => debug!(
                    "BIOS A memcpy dst: {:08x}, src: {:08x}, len: {:08x}",
                    self.regs[4], self.regs[5], self.regs[6]
                ),
                0x2F => debug!("BIOS A rand"),
                0x33 => debug!("BIOS A malloc size: {:08x}", self.regs[4]),
                0x39 => debug!(
                    "BIOS A InitHeap addr: {:08x}, size: {:08x}",
                    self.regs[4], self.regs[5]
                ),
                0x3F => debug!(
                    "BIOS A printf txt: {}, param1: {}, param2: {}",
                    self.debug_string(self.regs[4]),
                    self.regs[5],
                    self.regs[6]
                ),
                0x44 => debug!("BIOS A FlushCache"),
                0x49 => debug!("BIOS A GPU_cw gp0cmd: {:08x}", self.regs[4]),
                0x4A => debug!(
                    "BIOS A GPU_cwp gp0cmd: {:08x}, num: {:08x}",
                    self.regs[4], self.regs[5]
                ),
                0x4B => debug!("BIOS A send_gpu_linked_list({:08x})", self.regs[4]),
                0x5B => debug!("BIOS A dev_tty_init"),
                0x72 => debug!("BIOS A CdRemove"),
                0x96 => debug!("BIOS A AddCDROMDevice"),
                0x97 => debug!("BIOS A AddMemCardDevice"),
                0x99 => debug!("BIOS A AddDummyTtyDevice"),
                0xA3 => debug!("bIOS A DequeueCdIntr"),
                n => {
                    debug!("BIOS A {:08x}", n)
                }
            },
            0x000000B0 => match self.regs[9] {
                0x00 => debug!("BIOS B alloc_kernel_memory size: {:08x}", self.regs[4]),
                0x07 => debug!(
                    "BIOS B DeliverEvent class: {}, spec: {}",
                    self.debug_event_class(self.regs[4]),
                    self.debug_event_spec(self.regs[5])
                ),
                0x08 => debug!(
                    "BIOS B OpenEvent class: {}, spec: {}, mode: {}, func: {:08x}",
                    self.debug_event_class(self.regs[4]),
                    self.debug_event_spec(self.regs[5]),
                    self.debug_event_mode(self.regs[6]),
                    self.regs[7]
                ),
                0x09 => debug!("BIOS B CloseEvent {:08x}", self.regs[4]),
                0x0A => debug!("BIOS B WaitEvent {:08x}", self.regs[4]),
                0x0B => trace!("BIOS B TestEvent {:08x}", self.regs[4]),
                0x0C => debug!("BIOS B EnableEvent {:08x}", self.regs[4]),
                0x0D => debug!("BIOS B DisableEvent {:08x}", self.regs[4]),
                0x17 => debug!("BIOS B ReturnFromException"),
                0x18 => debug!("BIOS B SetDefaultExitFromException"),
                0x19 => debug!(
                    "BIOS B SetCustomExitFromException addr: {:08x}",
                    self.regs[4]
                ),
                0x3D => {
                    let c = (self.regs[4] as u8) as char;
                    debug!("BIOS B std_out_putchar {}", c);

                    if c as u8 == 0x0A {
                        info!("STDOUT: {}", self.tty_buffer);
                        self.tty_buffer.clear();
                    } else {
                        self.tty_buffer.push(c);
                    }
                }
                0x3F => {
                    debug!("BIOS B std_out_puts {}", self.debug_string(self.regs[4]));
                }
                0x47 => debug!("BIOS B AddDevice device_info: {:08x}", self.regs[4]),
                0x5B => debug!("BIOS B ChangeClearPad int: {:08x}", self.regs[4]),
                n => {
                    debug!("BIOS B {:08x}", n)
                }
            },
            0x000000C0 => match self.regs[9] {
                0x00 => debug!(
                    "BIOS C EnqueueTimerAndVblankIrqs priority: {}",
                    self.regs[4]
                ),
                0x01 => debug!("BIOS C EnqueueSyscallHandler priority: {}", self.regs[4]),
                0x02 => debug!(
                    "BIOS C SysEnqIntRP priority: {:08x}, struct: {:08x}",
                    self.regs[4], self.regs[5],
                ),
                0x03 => debug!(
                    "BIOS C SysDeqIntRP priority: {:08x}, struct: {:08x}",
                    self.regs[4], self.regs[5],
                ),
                0x07 => debug!("BIOS C InstallExceptionHandlers"),
                0x08 => debug!(
                    "BIOS C SysInitMemory addr: {:08x}, size: {:08x}",
                    self.regs[4], self.regs[5]
                ),
                0x0A => debug!(
                    "BIOS C ChangeClearRCnt t: {:08x}, flag: {:08x}",
                    self.regs[4], self.regs[5]
                ),
                0x0C => debug!("BIOS C InitDefInt priority: {}", self.regs[4]),
                0x12 => debug!("BIOS C InstallDevices ttyflag: {}", self.regs[4]),
                0x1C => debug!("BIOS C AdjustA0Table"),
                n => {
                    debug!("BIOS C {:08x}", n)
                }
            },
            _ => {}
        }
    }

    pub fn decode_and_execute(&mut self, instruction: Instruction) {
        trace!("decode_and_execute: {:08x}", instruction);

        self.stalls += 1;

        self.debug_bios_func();

        match instruction.function() {
            0b000000 => match instruction.subfunction() {
                0b000000 => self.op_sll(instruction),
                0b000010 => self.op_srl(instruction),
                0b000011 => self.op_sra(instruction),
                0b000100 => self.op_sllv(instruction),
                0b000110 => self.op_srlv(instruction),
                0b000111 => self.op_srav(instruction),
                0b001000 => self.op_jr(instruction),
                0b001001 => self.op_jalr(instruction),
                0b001100 => self.op_syscall(instruction),
                0b001101 => self.op_break(instruction),
                0b010000 => self.op_mfhi(instruction),
                0b010001 => self.op_mthi(instruction),
                0b010010 => self.op_mflo(instruction),
                0b010011 => self.op_mtlo(instruction),
                0b011000 => self.op_mult(instruction),
                0b011001 => self.op_multu(instruction),
                0b011010 => self.op_div(instruction),
                0b011011 => self.op_divu(instruction),
                0b100000 => self.op_add(instruction),
                0b100001 => self.op_addu(instruction),
                0b100010 => self.op_sub(instruction),
                0b100011 => self.op_subu(instruction),
                0b100100 => self.op_and(instruction),
                0b100101 => self.op_or(instruction),
                0b100110 => self.op_xor(instruction),
                0b100111 => self.op_nor(instruction),
                0b101010 => self.op_slt(instruction),
                0b101011 => self.op_sltu(instruction),
                _ => self.op_illegal(instruction),
            },
            0b000001 => self.op_bxx(instruction),
            0b000010 => self.op_j(instruction),
            0b000011 => self.op_jal(instruction),
            0b000100 => self.op_beq(instruction),
            0b000101 => self.op_bne(instruction),
            0b000110 => self.op_blez(instruction),
            0b000111 => self.op_bgtz(instruction),
            0b001000 => self.op_addi(instruction),
            0b001001 => self.op_addiu(instruction),
            0b001010 => self.op_slti(instruction),
            0b001011 => self.op_sltiu(instruction),
            0b001100 => self.op_andi(instruction),
            0b001101 => self.op_ori(instruction),
            0b001110 => self.op_xori(instruction),
            0b001111 => self.op_lui(instruction),
            0b010000 => self.op_cop0(instruction),
            0b010001 => self.op_cop1(instruction),
            0b010010 => self.op_cop2(instruction),
            0b010011 => self.op_cop3(instruction),
            0b100000 => self.op_lb(instruction),
            0b100001 => self.op_lh(instruction),
            0b100010 => self.op_lwl(instruction),
            0b100011 => self.op_lw(instruction),
            0b100100 => self.op_lbu(instruction),
            0b100101 => self.op_lhu(instruction),
            0b100110 => self.op_lwr(instruction),
            0b101000 => self.op_sb(instruction),
            0b101001 => self.op_sh(instruction),
            0b101010 => self.op_swl(instruction),
            0b101011 => self.op_sw(instruction),
            0b101110 => self.op_swr(instruction),
            0b110000 => self.op_lwc0(instruction),
            0b110001 => self.op_lwc1(instruction),
            0b110010 => self.op_lwc2(instruction),
            0b110011 => self.op_lwc3(instruction),
            0b111000 => self.op_swc0(instruction),
            0b111001 => self.op_swc1(instruction),
            0b111010 => self.op_swc2(instruction),
            0b111011 => self.op_swc3(instruction),
            _ => self.op_illegal(instruction),
        }
    }

    fn op_lui(&mut self, instruction: Instruction) {
        let i = instruction.imm();
        let t = instruction.t();

        let v = i << 16;

        self.set_reg(t, v);
    }

    fn op_ori(&mut self, instruction: Instruction) {
        let i = instruction.imm();
        let t = instruction.t();
        let s = instruction.s();

        let v = self.reg(s) | i;

        self.set_reg(t, v);
    }

    fn op_sll(&mut self, instruction: Instruction) {
        let i = instruction.shift();
        let t = instruction.t();
        let d = instruction.d();

        let v = self.reg(t) << i;

        self.set_reg(d, v);
    }

    fn op_or(&mut self, instruction: Instruction) {
        let d = instruction.d();
        let s = instruction.s();
        let t = instruction.t();

        let v = self.reg(s) | self.reg(t);

        self.set_reg(d, v);
    }

    fn op_sltu(&mut self, instruction: Instruction) {
        let d = instruction.d();
        let s = instruction.s();
        let t = instruction.t();

        let v = self.reg(s) < self.reg(t);

        self.set_reg(d, v as u32);
    }

    fn op_j(&mut self, instruction: Instruction) {
        let i = instruction.imm_jump();

        self.next_pc = (self.pc & 0xF0000000) | (i << 2);
        self.branch = true;
    }

    fn op_bne(&mut self, instruction: Instruction) {
        let i = instruction.imm_se();
        let s = instruction.s();
        let t = instruction.t();

        if self.reg(s) != self.reg(t) {
            self.branch(i);
        }
    }

    fn op_beq(&mut self, instruction: Instruction) {
        let i = instruction.imm_se();
        let s = instruction.s();
        let t = instruction.t();

        if self.reg(s) == self.reg(t) {
            self.branch(i);
        }
    }

    fn op_cop0(&mut self, instruction: Instruction) {
        match instruction.cop_opcode() {
            0b00000 => self.op_mfc0(instruction),
            0b00010 => self.op_cfc0(instruction),
            0b00100 => self.op_mtc0(instruction),
            0b00110 => self.op_ctc0(instruction),
            0b10000 => self.op_rfe(instruction),
            _ => panic!("unhandled cop0 instruction"),
        }
    }

    fn op_mtc0(&mut self, instruction: Instruction) {
        let cpu_r = instruction.t();
        let cop_r = instruction.d().0;

        let v = self.reg(cpu_r);

        match cop_r {
            3 | 5 | 6 | 7 | 9 | 11 => {
                if v != 0 {
                    panic!("Unhandled write to cop0r{}", cop_r);
                }
            }
            12 => self.sr = v,
            13 => self.set_cause(v),
            14 => self.epc = v,
            n => panic!("Unhandled cop0 register: {:08x}", n),
        }
    }

    fn op_mfc0(&mut self, instruction: Instruction) {
        let cpu_r = instruction.t();
        let cop_r = instruction.d().0;

        let v = match cop_r {
            12 => self.sr,
            13 => self.cause,
            14 => self.epc,
            15 => 0, // Processor ID (Read)
            _ => panic!("Unhandled read cop0 register: {:08x}", cop_r),
        };

        self.load = (cpu_r, v);
    }

    fn op_cfc0(&mut self, _: Instruction) {
        todo!()
    }

    fn op_ctc0(&mut self, _: Instruction) {
        todo!()
    }

    fn op_rfe(&mut self, instruction: Instruction) {
        if instruction.0 & 0x3F != 0b010000 {
            panic!("Invalid cop0 instruction: {:08x}", instruction.0);
        }

        let mode = self.sr & 0x3F;
        self.sr &= !0x3F;
        self.sr |= mode >> 2;
    }

    fn op_cop1(&mut self, _: Instruction) {
        self.exception(Exception::CoprocessorError);
    }

    fn op_cop2(&mut self, instruction: Instruction) {
        let op = instruction.cop_opcode();
        if op & 0x10 > 0 {
            return self.gte.command(instruction.imm_cop());
        }

        match op {
            0b00000 => self.op_mfc2(instruction),
            0b00010 => self.op_cfc2(instruction),
            0b00100 => self.op_mtc2(instruction),
            0b00110 => self.op_ctc2(instruction),
            _ => panic!("unhandled GTE instruction: {:08x}", instruction),
        }
    }

    fn op_mtc2(&mut self, instruction: Instruction) {
        let t = instruction.t();
        let d = instruction.d();

        let val = self.reg(t);

        self.gte.store_data(d, val);
    }

    fn op_mfc2(&mut self, instruction: Instruction) {
        let t = instruction.t();
        let d = instruction.d();

        let val = self.gte.load_data(d);

        self.set_reg(t, val);
    }

    fn op_cfc2(&mut self, instruction: Instruction) {
        let t = instruction.t();
        let d = instruction.d();

        let val = self.gte.load_control(d);

        self.set_reg(t, val);
    }

    fn op_ctc2(&mut self, instruction: Instruction) {
        let t = instruction.t();
        let d = instruction.d();

        let val = self.reg(t);

        self.gte.store_control(d, val);
    }

    fn op_cop3(&mut self, _: Instruction) {
        self.exception(Exception::CoprocessorError);
    }

    fn op_srl(&mut self, instruction: Instruction) {
        let i = instruction.shift();
        let t = instruction.t();
        let d = instruction.d();

        let v = self.reg(t) >> i;

        self.set_reg(d, v);
    }

    fn op_sra(&mut self, instruction: Instruction) {
        let i = instruction.shift();
        let t = instruction.t();
        let d = instruction.d();

        let v = (self.reg(t) as i32) >> i;

        self.set_reg(d, v as u32);
    }

    fn op_sllv(&mut self, instruction: Instruction) {
        let d = instruction.d();
        let s = instruction.s();
        let t = instruction.t();

        let v = self.reg(t) << (self.reg(s) & 0x1F);

        self.set_reg(d, v);
    }

    fn op_srlv(&mut self, instruction: Instruction) {
        let d = instruction.d();
        let s = instruction.s();
        let t = instruction.t();

        let v = self.reg(t) >> (self.reg(s) & 0x1F);

        self.set_reg(d, v);
    }

    fn op_srav(&mut self, instruction: Instruction) {
        let d = instruction.d();
        let s = instruction.s();
        let t = instruction.t();

        let v = (self.reg(t) as i32) >> (self.reg(s) & 0x1F);

        self.set_reg(d, v as u32);
    }

    fn op_jr(&mut self, instruction: Instruction) {
        let s = instruction.s();

        self.next_pc = self.reg(s);
        self.branch = true;
    }

    fn op_jalr(&mut self, instruction: Instruction) {
        let d = instruction.d();
        let s = instruction.s();

        let ra = self.next_pc;

        self.set_reg(d, ra);

        self.next_pc = self.reg(s);
        self.branch = true;
    }

    fn op_syscall(&mut self, instruction: Instruction) {
        debug!(
            "CPU syscall 0x{:08x} at {:08x}",
            instruction.0, self.current_pc
        );
        self.exception(Exception::SysCall)
    }

    fn op_break(&mut self, _: Instruction) {
        self.event = Some(Event::Break);
        self.exception(Exception::Break);
    }

    fn op_mfhi(&mut self, instruction: Instruction) {
        let d = instruction.d();

        let hi = self.hi;

        self.set_reg(d, hi);
    }

    fn op_mflo(&mut self, instruction: Instruction) {
        let d = instruction.d();

        let lo = self.lo;

        self.set_reg(d, lo);
    }

    fn op_mthi(&mut self, instruction: Instruction) {
        let s = instruction.s();

        self.hi = self.reg(s);
    }

    fn op_mtlo(&mut self, instruction: Instruction) {
        let s = instruction.s();

        self.lo = self.reg(s);
    }

    fn op_mult(&mut self, instruction: Instruction) {
        let s = instruction.s();
        let t = instruction.t();

        let a = (self.reg(s) as i32) as i64;
        let b = (self.reg(t) as i32) as i64;

        let v = a * b;

        self.hi = (v >> 32) as u32;
        self.lo = v as u32;
    }

    fn op_multu(&mut self, instruction: Instruction) {
        let s = instruction.s();
        let t = instruction.t();

        let a = self.reg(s) as u64;
        let b = self.reg(t) as u64;

        let v = a * b;

        self.hi = (v >> 32) as u32;
        self.lo = v as u32;
    }

    fn op_div(&mut self, instruction: Instruction) {
        let s = instruction.s();
        let t = instruction.t();

        let n = self.reg(s) as i32;
        let d = self.reg(t) as i32;

        if d == 0 {
            self.hi = n as u32;

            if n >= 0 {
                self.lo = 0xFFFFFFFF;
            } else {
                self.lo = 1;
            }
        } else if n as u32 == 0x80000000 && d == -1 {
            self.hi = 0;
            self.lo = 0x80000000;
        } else {
            self.hi = (n % d) as u32;
            self.lo = (n / d) as u32;
        }
    }

    fn op_divu(&mut self, instruction: Instruction) {
        let s = instruction.s();
        let t = instruction.t();

        let n = self.reg(s);
        let d = self.reg(t);

        if d == 0 {
            self.hi = n;
            self.lo = 0xFFFFFFFF;
        } else {
            self.hi = n % d;
            self.lo = n / d;
        }
    }

    fn op_add(&mut self, instruction: Instruction) {
        let s = instruction.s();
        let t = instruction.t();
        let d = instruction.d();

        let s = self.reg(s) as i32;
        let t = self.reg(t) as i32;

        match s.checked_add(t) {
            Some(v) => self.set_reg(d, v as u32),
            None => self.exception(Exception::Overflow),
        }
    }

    fn op_addu(&mut self, instruction: Instruction) {
        let s = instruction.s();
        let t = instruction.t();
        let d = instruction.d();

        let v = self.reg(s).wrapping_add(self.reg(t));

        self.set_reg(d, v);
    }

    fn op_addiu(&mut self, instruction: Instruction) {
        let i = instruction.imm_se();
        let t = instruction.t();
        let s = instruction.s();

        let v = self.reg(s).wrapping_add(i);

        self.set_reg(t, v);
    }

    fn op_addi(&mut self, instruction: Instruction) {
        let i = instruction.imm_se() as i32;
        let t = instruction.t();
        let s = instruction.s();

        let s = self.reg(s) as i32;

        match s.checked_add(i) {
            Some(v) => self.set_reg(t, v as u32),
            None => self.exception(Exception::Overflow),
        }
    }

    fn op_sub(&mut self, instruction: Instruction) {
        let s = instruction.s();
        let t = instruction.t();
        let d = instruction.d();

        let s = self.reg(s) as i32;
        let t = self.reg(t) as i32;

        match s.checked_sub(t) {
            Some(v) => self.set_reg(d, v as u32),
            None => self.exception(Exception::Overflow),
        }
    }

    fn op_subu(&mut self, instruction: Instruction) {
        let s = instruction.s();
        let t = instruction.t();
        let d = instruction.d();

        let v = self.reg(s).wrapping_sub(self.reg(t));

        self.set_reg(d, v);
    }

    fn op_and(&mut self, instruction: Instruction) {
        let d = instruction.d();
        let s = instruction.s();
        let t = instruction.t();

        let v = self.reg(s) & self.reg(t);

        self.set_reg(d, v);
    }

    fn op_xor(&mut self, instruction: Instruction) {
        let d = instruction.d();
        let s = instruction.s();
        let t = instruction.t();

        let v = self.reg(s) ^ self.reg(t);

        self.set_reg(d, v);
    }

    fn op_nor(&mut self, instruction: Instruction) {
        let d = instruction.d();
        let s = instruction.s();
        let t = instruction.t();

        let v = !(self.reg(s) | self.reg(t));

        self.set_reg(d, v);
    }

    fn op_slt(&mut self, instruction: Instruction) {
        let d = instruction.d();
        let s = instruction.s();
        let t = instruction.t();

        let s = self.reg(s) as i32;
        let t = self.reg(t) as i32;

        let v = s < t;

        self.set_reg(d, v as u32);
    }

    fn op_bxx(&mut self, instruction: Instruction) {
        let i = instruction.imm_se();
        let s = instruction.s();

        let Instruction(instruction) = instruction;

        let is_bgez = (instruction >> 16) & 1;
        let is_link = (instruction >> 17) & 0xF == 8;

        let v = self.reg(s) as i32;

        let test = (v < 0) as u32;

        let test = test ^ is_bgez;

        if is_link {
            let ra = self.next_pc;

            self.set_reg(RegisterIndex(31), ra);
        }

        if test != 0 {
            self.branch(i);
        }
    }

    fn op_jal(&mut self, instruction: Instruction) {
        let ra = self.next_pc;

        self.set_reg(RegisterIndex(31), ra);

        self.op_j(instruction);
    }

    fn op_blez(&mut self, instruction: Instruction) {
        let i = instruction.imm_se();
        let s = instruction.s();

        let v = self.reg(s) as i32;

        if v <= 0 {
            self.branch(i);
        }
    }

    fn op_bgtz(&mut self, instruction: Instruction) {
        let i = instruction.imm_se();
        let s = instruction.s();

        let v = self.reg(s) as i32;

        if v > 0 {
            self.branch(i);
        }
    }

    fn op_slti(&mut self, instruction: Instruction) {
        let i = instruction.imm_se() as i32;
        let s = instruction.s();
        let t = instruction.t();

        let v = (self.reg(s) as i32) < i;

        self.set_reg(t, v as u32);
    }

    fn op_sltiu(&mut self, instruction: Instruction) {
        let i = instruction.imm_se();
        let s = instruction.s();
        let t = instruction.t();

        let v = self.reg(s) < i;

        self.set_reg(t, v as u32);
    }

    fn op_andi(&mut self, instruction: Instruction) {
        let i = instruction.imm();
        let t = instruction.t();
        let s = instruction.s();

        let v = self.reg(s) & i;

        self.set_reg(t, v);
    }

    fn op_xori(&mut self, instruction: Instruction) {
        let i = instruction.imm();
        let t = instruction.t();
        let s = instruction.s();

        let v = self.reg(s) ^ i;

        self.set_reg(t, v);
    }

    fn op_lb(&mut self, instruction: Instruction) {
        let i = instruction.imm_se();
        let t = instruction.t();
        let s = instruction.s();

        let addr = self.reg(s).wrapping_add(i);

        let v = self.load::<u8>(addr) as i8;

        self.load = (t, v as u32);
    }

    fn op_lh(&mut self, instruction: Instruction) {
        let i = instruction.imm_se();
        let t = instruction.t();
        let s = instruction.s();

        let addr = self.reg(s).wrapping_add(i);

        if addr % 2 == 0 {
            let v = self.load::<u16>(addr) as i16;

            self.load = (t, v as u32);
        } else {
            self.exception(Exception::LoadAddressError);
        }
    }

    fn op_lw(&mut self, instruction: Instruction) {
        let i = instruction.imm_se();
        let t = instruction.t();
        let s = instruction.s();

        let addr = self.reg(s).wrapping_add(i);

        if addr % 4 == 0 {
            let v = self.load::<u32>(addr);

            self.load = (t, v);
        } else {
            self.exception(Exception::LoadAddressError);
        }
    }

    fn op_lwl(&mut self, instruction: Instruction) {
        let i = instruction.imm_se();
        let t = instruction.t();
        let s = instruction.s();

        let addr = self.reg(s).wrapping_add(i);

        let cur_v = self.out_regs[t.0 as usize];

        let aligned_addr = addr & !3;
        let aligned_word = self.load::<u32>(aligned_addr);

        let v = match addr & 3 {
            0 => (cur_v & 0x00FFFFFF) | (aligned_word << 24),
            1 => (cur_v & 0x0000FFFF) | (aligned_word << 16),
            2 => (cur_v & 0x000000FF) | (aligned_word << 8),
            3 => (cur_v & 0x00000000) | (aligned_word << 0),
            _ => unreachable!(),
        };

        self.load = (t, v);
    }

    fn op_lbu(&mut self, instruction: Instruction) {
        let i = instruction.imm_se();
        let t = instruction.t();
        let s = instruction.s();

        let addr = self.reg(s).wrapping_add(i);

        let v = self.load::<u8>(addr);

        self.load = (t, v as u32);
    }

    fn op_lhu(&mut self, instruction: Instruction) {
        let i = instruction.imm_se();
        let t = instruction.t();
        let s = instruction.s();

        let addr = self.reg(s).wrapping_add(i);

        if addr % 2 == 0 {
            let v = self.load::<u16>(addr);

            self.load = (t, v as u32);
        } else {
            self.exception(Exception::LoadAddressError);
        }
    }

    fn op_lwr(&mut self, instruction: Instruction) {
        let i = instruction.imm_se();
        let t = instruction.t();
        let s = instruction.s();

        let addr = self.reg(s).wrapping_add(i);

        let cur_v = self.out_regs[t.0 as usize];

        let aligned_addr = addr & !3;
        let aligned_word = self.load::<u32>(aligned_addr);

        let v = match addr & 3 {
            0 => (cur_v & 0x00000000) | (aligned_word >> 0),
            1 => (cur_v & 0xFF000000) | (aligned_word >> 8),
            2 => (cur_v & 0xFFFF0000) | (aligned_word >> 16),
            3 => (cur_v & 0xFFFFFF00) | (aligned_word >> 24),
            _ => unreachable!(),
        };

        self.load = (t, v);
    }

    fn op_sb(&mut self, instruction: Instruction) {
        if self.sr & 0x10000 != 0 {
            // warn!("Ignoring store while cache is isolated");
            return;
        }

        let i = instruction.imm_se();
        let t = instruction.t();
        let s = instruction.s();

        let addr = self.reg(s).wrapping_add(i);
        let v = self.reg(t);

        self.store::<u8>(addr, v as u8);
    }

    fn op_sh(&mut self, instruction: Instruction) {
        if self.sr & 0x10000 != 0 {
            warn!("Ignoring store while cache is isolatee");
            return;
        }

        let i = instruction.imm_se();
        let t = instruction.t();
        let s = instruction.s();

        let addr = self.reg(s).wrapping_add(i);

        if addr % 2 == 0 {
            let v = self.reg(t);

            self.store::<u16>(addr, v as u16);
        } else {
            self.exception(Exception::StoreAddressError);
        }
    }

    fn op_sw(&mut self, instruction: Instruction) {
        if self.sr & 0x10000 != 0 {
            // warn!("ignoring store while cache is isolated");
            return;
        }

        let i = instruction.imm_se();
        let t = instruction.t();
        let s = instruction.s();

        let addr = self.reg(s).wrapping_add(i);

        if addr % 4 == 0 {
            let v = self.reg(t);

            self.store::<u32>(addr, v);
        } else {
            self.exception(Exception::StoreAddressError);
        }
    }

    fn op_swl(&mut self, instruction: Instruction) {
        let i = instruction.imm_se();
        let t = instruction.t();
        let s = instruction.s();

        let addr = self.reg(s).wrapping_add(i);
        let v = self.reg(t);

        let aligned_addr = addr & !3;
        let cur_mem = self.load::<u32>(aligned_addr);

        let mem = match addr & 3 {
            0 => (cur_mem & 0xFFFFFF00) | (v >> 24),
            1 => (cur_mem & 0xFFFF0000) | (v >> 16),
            2 => (cur_mem & 0xFF000000) | (v >> 8),
            3 => (cur_mem & 0x00000000) | (v >> 0),
            _ => unreachable!(),
        };

        self.store::<u32>(aligned_addr, mem);
    }

    fn op_swr(&mut self, instruction: Instruction) {
        let i = instruction.imm_se();
        let t = instruction.t();
        let s = instruction.s();

        let addr = self.reg(s).wrapping_add(i);
        let v = self.reg(t);

        let aligned_addr = addr & !3;
        let cur_mem = self.load::<u32>(aligned_addr);

        let mem = match addr & 3 {
            0 => (cur_mem & 0x00000000) | (v << 0),
            1 => (cur_mem & 0x000000FF) | (v << 8),
            2 => (cur_mem & 0x0000FFFF) | (v << 16),
            3 => (cur_mem & 0x00FFFFFF) | (v << 24),
            _ => unreachable!(),
        };

        self.store::<u32>(aligned_addr, mem);
    }

    fn op_lwc0(&mut self, _: Instruction) {
        self.exception(Exception::CoprocessorError);
    }

    fn op_lwc1(&mut self, _: Instruction) {
        self.exception(Exception::CoprocessorError);
    }

    fn op_lwc2(&mut self, instruction: Instruction) {
        panic!("unhandled GTE LWC: {:08x}", instruction);
    }

    fn op_lwc3(&mut self, _: Instruction) {
        self.exception(Exception::CoprocessorError);
    }

    fn op_swc0(&mut self, _: Instruction) {
        self.exception(Exception::CoprocessorError);
    }

    fn op_swc1(&mut self, _: Instruction) {
        self.exception(Exception::CoprocessorError);
    }

    fn op_swc2(&mut self, instruction: Instruction) {
        panic!("unhandled GTE SWC: {:08x}", instruction);
    }

    fn op_swc3(&mut self, _: Instruction) {
        self.exception(Exception::CoprocessorError);
    }

    fn op_illegal(&mut self, _: Instruction) {
        self.exception(Exception::IllegalInstruction);
    }
}
