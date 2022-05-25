use super::cpu::{Cpu, ExecMode};

use gdbstub::target::ext::base::single_register_access::SingleRegisterAccess;
use gdbstub::target::ext::base::singlethread::SingleThreadBase;
use gdbstub::target::ext::breakpoints::{
    Breakpoints, HwWatchpoint, SwBreakpoint, SwBreakpointOps, WatchKind,
};
use gdbstub::target::ext::exec_file::ExecFile;
use gdbstub::target::ext::host_io::{
    FsKind, HostIo, HostIoClose, HostIoErrno, HostIoError, HostIoFstat, HostIoOpen,
    HostIoOpenFlags, HostIoOpenMode, HostIoPread, HostIoPwrite, HostIoReadlink, HostIoResult,
    HostIoSetfs, HostIoStat, HostIoUnlink,
};
use gdbstub::target::ext::memory_map::MemoryMap;
use gdbstub::target::{self, Target, TargetError, TargetResult};
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
    fn support_host_io(&mut self) -> Option<target::ext::host_io::HostIoOps<'_, Self>> {
        Some(self)
    }

    //#[inline(always)]
    //fn support_memory_map(&mut self) -> Option<target::ext::memory_map::MemoryMapOps<'_, Self>> {
    //    Some(self)
    //}

    #[inline(always)]
    fn support_exec_file(&mut self) -> Option<target::ext::exec_file::ExecFileOps<'_, Self>> {
        Some(self)
    }

    fn guard_rail_single_step_gdb_behavior(&self) -> gdbstub::arch::SingleStepGdbBehavior {
        gdbstub::arch::SingleStepGdbBehavior::Optional
    }
}

impl HostIo for Cpu {
    #[inline(always)]
    fn support_open(&mut self) -> Option<target::ext::host_io::HostIoOpenOps<'_, Self>> {
        Some(self)
    }

    #[inline(always)]
    fn support_close(&mut self) -> Option<target::ext::host_io::HostIoCloseOps<'_, Self>> {
        Some(self)
    }

    #[inline(always)]
    fn support_pread(&mut self) -> Option<target::ext::host_io::HostIoPreadOps<'_, Self>> {
        Some(self)
    }

    #[inline(always)]
    fn support_pwrite(&mut self) -> Option<target::ext::host_io::HostIoPwriteOps<'_, Self>> {
        Some(self)
    }

    #[inline(always)]
    fn support_fstat(&mut self) -> Option<target::ext::host_io::HostIoFstatOps<'_, Self>> {
        Some(self)
    }

    #[inline(always)]
    fn support_unlink(&mut self) -> Option<target::ext::host_io::HostIoUnlinkOps<'_, Self>> {
        Some(self)
    }

    #[inline(always)]
    fn support_readlink(&mut self) -> Option<target::ext::host_io::HostIoReadlinkOps<'_, Self>> {
        Some(self)
    }

    #[inline(always)]
    fn support_setfs(&mut self) -> Option<target::ext::host_io::HostIoSetfsOps<'_, Self>> {
        Some(self)
    }
}

impl HostIoOpen for Cpu {
    fn open(
        &mut self,
        filename: &[u8],
        _flags: HostIoOpenFlags,
        _mode: HostIoOpenMode,
    ) -> HostIoResult<u32, Self> {
        if filename == b"/test.rom" {
            return Ok(0);
        }

        return Err(HostIoError::Errno(HostIoErrno::ENOENT));
    }
}

impl HostIoClose for Cpu {
    fn close(&mut self, _fd: u32) -> HostIoResult<(), Self> {
        Ok(())
    }
}

impl HostIoPread for Cpu {
    fn pread<'a>(
        &mut self,
        fd: u32,
        count: usize,
        offset: u64,
        buf: &mut [u8],
    ) -> HostIoResult<usize, Self> {
        if fd == 0 {
            return Ok(copy_range_to_buf(&self.inter.bios.data, offset, count, buf));
        } else {
            return Err(HostIoError::Errno(HostIoErrno::EBADF));
        }
    }
}

impl HostIoPwrite for Cpu {
    fn pwrite(&mut self, _fd: u32, _offset: u32, _data: &[u8]) -> HostIoResult<u32, Self> {
        return Err(HostIoError::Errno(HostIoErrno::EACCES));
    }
}

impl HostIoFstat for Cpu {
    fn fstat(&mut self, fd: u32) -> HostIoResult<HostIoStat, Self> {
        if fd == 0 {
            return Ok(HostIoStat {
                st_dev: 0,
                st_ino: 0,
                st_mode: HostIoOpenMode::empty(),
                st_nlink: 0,
                st_uid: 0,
                st_gid: 0,
                st_rdev: 0,
                st_size: self.inter.bios.data.len() as u64,
                st_blksize: 0,
                st_blocks: 0,
                st_atime: 0,
                st_mtime: 0,
                st_ctime: 0,
            });
        } else {
            return Err(HostIoError::Errno(HostIoErrno::EBADF));
        }
    }
}

impl HostIoUnlink for Cpu {
    fn unlink(&mut self, _filename: &[u8]) -> HostIoResult<(), Self> {
        Ok(())
    }
}

impl HostIoReadlink for Cpu {
    fn readlink<'a>(&mut self, filename: &[u8], buf: &mut [u8]) -> HostIoResult<usize, Self> {
        if filename == b"/proc/1/exe" {
            let exe = b"/test.rom";
            return Ok(copy_to_buf(exe, buf));
        } else if filename == b"/proc/1/cwd" {
            let cwd = b"/";
            return Ok(copy_to_buf(cwd, buf));
        }

        return Err(HostIoError::Errno(HostIoErrno::ENOENT));
    }
}

impl HostIoSetfs for Cpu {
    fn setfs(&mut self, _fs: FsKind) -> HostIoResult<(), Self> {
        Ok(())
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

impl MemoryMap for Cpu {
    fn memory_map_xml(
        &self,
        offset: u64,
        length: usize,
        buf: &mut [u8],
    ) -> TargetResult<usize, Self> {
        let memory_map = r#"<?xml version="1.0"?>
<!DOCTYPE memory-map
    PUBLIC "+//IDN gnu.org//DTD GDB Memory Map V1.0//EN"
            "http://sourceware.org/gdb/gdb-memory-map.dtd">
<memory-map>
    <memory type="ram" start="0x0" length="0x100000000"/>
</memory-map>"#
            .trim()
            .as_bytes();
        Ok(copy_range_to_buf(memory_map, offset, length, buf))
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
        for i in (0..data.len()).step_by(4) {
            let word = self.examine::<u32>(start_addr + i as u32);

            for j in 0..4 {
                if i + j < data.len() {
                    data[i + j] = (word >> (8 * j)) as u8;
                }
            }
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
    fn support_single_step(
        &mut self,
    ) -> Option<target::ext::base::singlethread::SingleThreadSingleStepOps<'_, Self>> {
        Some(self)
    }

    #[inline(always)]
    fn support_range_step(
        &mut self,
    ) -> Option<target::ext::base::singlethread::SingleThreadRangeSteppingOps<'_, Self>> {
        Some(self)
    }
}

impl target::ext::base::singlethread::SingleThreadSingleStep for Cpu {
    fn step(&mut self, _signal: Option<gdbstub::common::Signal>) -> Result<(), Self::Error> {
        self.exec_mode = ExecMode::Step;
        Ok(())
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
