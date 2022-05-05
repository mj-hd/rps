use log::trace;

use crate::addressible::Addressible;

pub struct ScratchPad {
    data: Vec<u8>,
}

impl ScratchPad {
    pub fn new() -> ScratchPad {
        let data = [0xCA; 2 * 1024 * 1024].to_vec();

        ScratchPad { data }
    }

    pub fn load<T: Addressible>(&self, offset: u32) -> T {
        let offset = offset as usize;

        let mut v = 0;

        for i in 0..T::width() as usize {
            v |= (self.data[offset + i] as u32) << (i * 8);
        }

        trace!(
            "SCRATCHPAD{:?} load {:08x} => {:08x}",
            T::width(),
            offset,
            v
        );

        Addressible::from_u32(v)
    }

    pub fn store<T: Addressible>(&mut self, offset: u32, val: T) {
        let offset = offset as usize;

        trace!(
            "SCRATCHPAD{:?} store {:08x} => {:08x}",
            T::width(),
            offset,
            val.as_u32()
        );

        let val = val.as_u32();

        for i in 0..T::width() as usize {
            self.data[offset + i] = (val >> (i * 8)) as u8;
        }
    }
}
