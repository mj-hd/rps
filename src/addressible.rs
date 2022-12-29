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
    fn unwrap_u16(&self) -> u16;
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

    fn unwrap_u16(&self) -> u16 {
        panic!("invalid u16 unwrap of u8")
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

    fn unwrap_u16(&self) -> u16 {
        *self
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

    fn unwrap_u16(&self) -> u16 {
        panic!("invalid u16 unwrap of u32");
    }
}
