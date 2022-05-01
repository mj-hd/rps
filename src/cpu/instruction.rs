use std::fmt::LowerHex;

use super::RegisterIndex;

#[derive(Clone, Copy, Debug)]
pub struct Instruction(pub u32);

impl LowerHex for Instruction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Instruction {
    pub fn function(self) -> u32 {
        let Instruction(op) = self;

        op >> 26
    }

    pub fn s(self) -> RegisterIndex {
        let Instruction(op) = self;

        RegisterIndex((op >> 21) & 0x1F)
    }

    pub fn t(self) -> RegisterIndex {
        let Instruction(op) = self;

        RegisterIndex((op >> 16) & 0x1F)
    }

    pub fn d(self) -> RegisterIndex {
        let Instruction(op) = self;

        RegisterIndex((op >> 11) & 0x1F)
    }

    pub fn subfunction(self) -> u32 {
        let Instruction(op) = self;

        op & 0x3F
    }

    pub fn shift(self) -> u32 {
        let Instruction(op) = self;

        (op >> 6) & 0x1F
    }

    pub fn imm(self) -> u32 {
        let Instruction(op) = self;

        op & 0xFFFF
    }

    pub fn imm_se(self) -> u32 {
        let Instruction(op) = self;

        let v = (op & 0xFFFF) as i16;

        v as u32
    }

    pub fn imm_jump(self) -> u32 {
        let Instruction(op) = self;

        op & 0x3FFFFFF
    }

    pub fn cop_opcode(self) -> u32 {
        self.s().0
    }
}
