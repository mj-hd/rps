use log::{debug, trace, warn};

use crate::{addressible::Addressible, interconnect::Interconnect};

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
    RangeStep(u32, u32),
}

enum Exception {
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
    inter: Interconnect,
    load: (RegisterIndex, u32),
    branch: bool,
    delay_slot: bool,

    // COP0
    pub sr: u32,
    pub hi: u32,
    pub lo: u32,
    current_pc: u32,
    pub cause: u32,
    pub epc: u32,

    pub exec_mode: ExecMode,
    pub breakpoints: Vec<u32>,
    pub watchpoints: Vec<u32>,
    event: Option<Event>,
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
            exec_mode: ExecMode::Continue,
            breakpoints: vec![],
            watchpoints: vec![],
            event: None,
        }
    }

    fn reg(&self, index: RegisterIndex) -> u32 {
        self.regs[index.0 as usize]
    }

    fn set_reg(&mut self, index: RegisterIndex, val: u32) {
        self.out_regs[index.0 as usize] = val;

        self.out_regs[0] = 0;
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
                        break RunEvent::Event(event);
                    }
                }
            }
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
        self.event = None;

        self.current_pc = self.pc;

        if self.current_pc % 4 != 0 {
            self.exception(Exception::LoadAddressError);
            return self.event;
        }

        let instruction = Instruction(self.load::<u32>(self.pc));

        self.pc = self.next_pc;
        self.next_pc = self.next_pc.wrapping_add(4);

        let (reg, val) = self.load;
        self.set_reg(reg, val);

        self.load = (RegisterIndex(0), 0);
        self.delay_slot = self.branch;
        self.branch = false;

        self.decode_and_execute(instruction);

        self.regs = self.out_regs;

        if self.breakpoints.contains(&self.pc) {
            debug!("BREAK {:08x}", self.pc);
            self.event = Some(Event::Break);
            return self.event;
        }

        return self.event;
    }

    pub fn pc(&self) -> u32 {
        self.pc
    }

    pub fn load<T: Addressible>(&mut self, addr: u32) -> T {
        if self.watchpoints.contains(&addr) {
            self.event = Some(Event::WatchRead(addr));
        }
        self.inter.load(addr)
    }

    pub fn store<T: Addressible>(&mut self, addr: u32, val: T) {
        if self.watchpoints.contains(&addr) {
            self.event = Some(Event::WatchWrite(addr));
        }
        if self.sr & 0x10000 != 0 {
            warn!("Ignoring store while cache is isolated");
            return;
        }
        self.inter.store(addr, val)
    }

    pub fn examine<T: Addressible>(&mut self, addr: u32) -> T {
        self.inter.load(addr)
    }

    pub fn put<T: Addressible>(&mut self, addr: u32, val: T) {
        self.inter.store(addr, val);
    }

    fn branch(&mut self, offset: u32) {
        let offset = offset << 2;

        self.next_pc = self.pc.wrapping_add(offset);
        self.branch = true;
    }

    fn exception(&mut self, cause: Exception) {
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

    pub fn decode_and_execute(&mut self, instruction: Instruction) {
        trace!("decode_and_execute: {:08x}", instruction);

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
            13 => self.cause = v,
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
        panic!("unhandled GTE instruction: {:08x}", instruction);
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

    fn op_syscall(&mut self, _: Instruction) {
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
            warn!("Ignoring store while cache is isolated");
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
            warn!("ignoring store while cache is isolated");
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
