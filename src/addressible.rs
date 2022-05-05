#[derive(PartialEq, Eq, Debug)]
pub enum AccessWidth {
    Byte = 1,
    Halfword = 2,
    Word = 4,
}

pub trait Addressible {
    fn width() -> AccessWidth;
    fn from_u32(val: u32) -> Self;
    fn as_u32(&self) -> u32;
}

impl Addressible for u8 {
    fn width() -> AccessWidth {
        AccessWidth::Byte
    }

    fn from_u32(val: u32) -> Self {
        val as u8
    }

    fn as_u32(&self) -> u32 {
        *self as u32
    }
}

impl Addressible for u16 {
    fn width() -> AccessWidth {
        AccessWidth::Halfword
    }

    fn from_u32(val: u32) -> Self {
        val as u16
    }

    fn as_u32(&self) -> u32 {
        *self as u32
    }
}

impl Addressible for u32 {
    fn width() -> AccessWidth {
        AccessWidth::Word
    }

    fn from_u32(val: u32) -> Self {
        val
    }

    fn as_u32(&self) -> u32 {
        *self
    }
}
