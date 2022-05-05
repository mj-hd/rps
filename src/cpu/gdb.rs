use super::cpu::{Cpu, ExecMode};

use gdbstub::target::ext::base::single_register_access::SingleRegisterAccess;
use gdbstub::target::ext::base::singlethread::SingleThreadBase;
use gdbstub::target::ext::breakpoints::{
    Breakpoints, HwWatchpoint, SwBreakpoint, SwBreakpointOps, WatchKind,
};
use gdbstub::target::ext::exec_file::ExecFile;
use gdbstub::target::ext::memory_map::MemoryMap;
use gdbstub::target::{self, Target, TargetError};
use gdbstub_arch::mips;
use log::debug;

pub fn copy_to_buf(data: &[u8], buf: &mut [u8]) -> usize {
    let len = buf.len().min(data.len());
    buf[..len].copy_from_slice(&data[..len]);
    len
}

pub fn copy_range_to_buf(data: &[u8], offset: u64, length: usize, buf: &mut [u8]) -> usize {
    let offset = offset as usize;
    if offset > data.len() {
        return 0;
    }

    let start = offset;
    let end = (offset + length).min(data.len());
    copy_to_buf(&data[start..end], buf)
}

impl Target for Cpu {
    type Arch = mips::Mips;
    type Error = &'static str;

    #[inline(always)]
    fn base_ops(&mut self) -> target::ext::base::BaseOps<'_, Self::Arch, Self::Error> {
        target::ext::base::BaseOps::SingleThread(self)
    }

    #[inline(always)]
    fn support_breakpoints(
        &mut self,
    ) -> Option<target::ext::breakpoints::BreakpointsOps<'_, Self>> {
        Some(self)
    }

    #[inline(always)]
    fn support_exec_file(&mut self) -> Option<target::ext::exec_file::ExecFileOps<'_, Self>> {
        Some(self)
    }
}

impl ExecFile for Cpu {
    fn get_exec_file(
        &self,
        _: Option<gdbstub::common::Pid>,
        offset: u64,
        length: usize,
        buf: &mut [u8],
    ) -> target::TargetResult<usize, Self> {
        let filename = b"/test.rom";
        Ok(copy_range_to_buf(filename, offset, length, buf))
    }
}

impl Breakpoints for Cpu {
    #[inline(always)]
    fn support_sw_breakpoint(&mut self) -> Option<SwBreakpointOps<'_, Self>> {
        Some(self)
    }

    #[inline(always)]
    fn support_hw_watchpoint(
        &mut self,
    ) -> Option<target::ext::breakpoints::HwWatchpointOps<'_, Self>> {
        Some(self)
    }
}

impl SwBreakpoint for Cpu {
    fn add_sw_breakpoint(
        &mut self,
        addr: <Self::Arch as gdbstub::arch::Arch>::Usize,
        _: <Self::Arch as gdbstub::arch::Arch>::BreakpointKind,
    ) -> target::TargetResult<bool, Self> {
        if !self.breakpoints.contains(&addr) {
            debug!("add breakpoint: {:08x}", addr);
            self.breakpoints.push(addr);
            return Ok(true);
        }

        Ok(false)
    }

    fn remove_sw_breakpoint(
        &mut self,
        addr: <Self::Arch as gdbstub::arch::Arch>::Usize,
        _: <Self::Arch as gdbstub::arch::Arch>::BreakpointKind,
    ) -> target::TargetResult<bool, Self> {
        if self.breakpoints.contains(&addr) {
            debug!("remove breakpoint: {:08x}", addr);
            self.breakpoints.retain(|&a| a != addr);
            return Ok(true);
        }

        Ok(false)
    }
}

impl HwWatchpoint for Cpu {
    fn add_hw_watchpoint(
        &mut self,
        addr: <Self::Arch as gdbstub::arch::Arch>::Usize,
        len: <Self::Arch as gdbstub::arch::Arch>::Usize,
        kind: target::ext::breakpoints::WatchKind,
    ) -> target::TargetResult<bool, Self> {
        for addr in addr..(addr + len) {
            match kind {
                WatchKind::Write => self.watchpoints.push(addr),
                WatchKind::Read => self.watchpoints.push(addr),
                WatchKind::ReadWrite => self.watchpoints.push(addr),
            }
        }

        Ok(true)
    }

    fn remove_hw_watchpoint(
        &mut self,
        addr: <Self::Arch as gdbstub::arch::Arch>::Usize,
        len: <Self::Arch as gdbstub::arch::Arch>::Usize,
        kind: target::ext::breakpoints::WatchKind,
    ) -> target::TargetResult<bool, Self> {
        for addr in addr..(addr + len) {
            let pos = match self.watchpoints.iter().position(|x| *x == addr) {
                None => return Ok(false),
                Some(pos) => pos,
            };

            match kind {
                WatchKind::Write => self.watchpoints.remove(pos),
                WatchKind::Read => self.watchpoints.remove(pos),
                WatchKind::ReadWrite => self.watchpoints.remove(pos),
            };
        }

        Ok(true)
    }
}

impl SingleThreadBase for Cpu {
    fn read_registers(
        &mut self,
        regs: &mut <Self::Arch as gdbstub::arch::Arch>::Registers,
    ) -> target::TargetResult<(), Self> {
        regs.r = self.regs;
        regs.hi = self.hi;
        regs.lo = self.lo;
        regs.pc = self.pc;
        regs.cp0.cause = self.cause;
        regs.cp0.status = self.sr;

        Ok(())
    }

    fn write_registers(
        &mut self,
        regs: &<Self::Arch as gdbstub::arch::Arch>::Registers,
    ) -> target::TargetResult<(), Self> {
        self.regs = regs.r;
        self.hi = regs.hi;
        self.lo = regs.lo;
        self.pc = regs.pc;
        self.cause = regs.cp0.cause;
        self.sr = regs.cp0.status;

        Ok(())
    }

    fn read_addrs(
        &mut self,
        start_addr: <Self::Arch as gdbstub::arch::Arch>::Usize,
        data: &mut [u8],
    ) -> target::TargetResult<(), Self> {
        for (addr, val) in (start_addr..).zip(data.iter_mut()) {
            *val = self.examine(addr);
        }

        Ok(())
    }

    fn write_addrs(
        &mut self,
        start_addr: <Self::Arch as gdbstub::arch::Arch>::Usize,
        data: &[u8],
    ) -> target::TargetResult<(), Self> {
        for (addr, val) in (start_addr..).zip(data.iter().copied()) {
            self.put(addr, val);
        }

        Ok(())
    }

    #[inline(always)]
    fn support_resume(
        &mut self,
    ) -> Option<target::ext::base::singlethread::SingleThreadResumeOps<'_, Self>> {
        Some(self)
    }

    #[inline(always)]
    fn support_single_register_access(
        &mut self,
    ) -> Option<target::ext::base::single_register_access::SingleRegisterAccessOps<'_, (), Self>>
    {
        Some(self)
    }
}

impl target::ext::base::singlethread::SingleThreadResume for Cpu {
    fn resume(&mut self, signal: Option<gdbstub::common::Signal>) -> Result<(), Self::Error> {
        if signal.is_some() {
            return Err("Unsupported signal");
        }

        self.exec_mode = ExecMode::Continue;

        Ok(())
    }

    #[inline(always)]
    fn support_range_step(
        &mut self,
    ) -> Option<target::ext::base::singlethread::SingleThreadRangeSteppingOps<'_, Self>> {
        Some(self)
    }
}

impl target::ext::base::singlethread::SingleThreadRangeStepping for Cpu {
    fn resume_range_step(&mut self, start: u32, end: u32) -> Result<(), Self::Error> {
        self.exec_mode = ExecMode::RangeStep(start, end);
        Ok(())
    }
}

impl SingleRegisterAccess<()> for Cpu {
    fn read_register(
        &mut self,
        _tid: (),
        reg_id: <Self::Arch as gdbstub::arch::Arch>::RegId,
        buf: &mut [u8],
    ) -> target::TargetResult<usize, Self> {
        match reg_id {
            mips::reg::id::MipsRegId::Gpr(reg_id) => {
                buf.copy_from_slice(&self.regs[reg_id as usize].to_le_bytes());
                Ok(buf.len())
            }
            _ => Err(TargetError::Fatal("Unsupported register")),
        }
    }

    fn write_register(
        &mut self,
        _tid: (),
        reg_id: <Self::Arch as gdbstub::arch::Arch>::RegId,
        val: &[u8],
    ) -> target::TargetResult<(), Self> {
        let val = u32::from_le_bytes(
            val.try_into()
                .map_err(|_| TargetError::Fatal("invalid data"))?,
        );
        match reg_id {
            mips::reg::id::MipsRegId::Gpr(reg_id) => {
                self.regs[reg_id as usize] = val;
                Ok(())
            }
            _ => Err(TargetError::Fatal("Unsupported register")),
        }
    }
}
