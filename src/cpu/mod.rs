#[derive(Clone, Copy)]
pub struct RegisterIndex(u32);

pub mod cpu;
pub mod gdb;
mod instruction;
