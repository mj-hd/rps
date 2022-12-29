#[derive(Clone, Copy)]
pub struct RegisterIndex(pub u32);

pub mod cpu;
pub mod gdb;
mod instruction;
