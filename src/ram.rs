use log::trace;

pub struct Ram {
    data: Vec<u8>,
}

impl Ram {
    pub fn new() -> Ram {
        let data = [0xCA; 2 * 1024 * 1024].to_vec();

        Ram { data }
    }

    pub fn load32(&self, offset: u32) -> u32 {
        let offset = offset as usize;

        let b0 = self.data[offset + 0] as u32;
        let b1 = self.data[offset + 1] as u32;
        let b2 = self.data[offset + 2] as u32;
        let b3 = self.data[offset + 3] as u32;

        let result = b0 | (b1 << 8) | (b2 << 16) | (b3 << 24);

        trace!("RAM load32 {:08x} => {:08x}", offset, result);

        result
    }

    pub fn load16(&self, offset: u32) -> u16 {
        let offset = offset as usize;

        let b0 = self.data[offset + 0] as u16;
        let b1 = self.data[offset + 1] as u16;

        let result = b0 | (b1 << 8);

        trace!("RAM load16 {:08x} => {:04x}", offset, result);

        result
    }

    pub fn load8(&self, offset: u32) -> u8 {
        let offset = offset as usize;

        let result = self.data[offset];

        trace!("RAM load8 {:08x} => {:02x}", offset, result);

        result
    }

    pub fn store32(&mut self, offset: u32, val: u32) {
        let offset = offset as usize;

        trace!("RAM store32 {:08x} => {:08x}", offset, val);

        let b0 = val >> 0;
        let b1 = val >> 8;
        let b2 = val >> 16;
        let b3 = val >> 24;

        self.data[offset + 0] = b0 as u8;
        self.data[offset + 1] = b1 as u8;
        self.data[offset + 2] = b2 as u8;
        self.data[offset + 3] = b3 as u8;
    }

    pub fn store16(&mut self, offset: u32, val: u16) {
        let offset = offset as usize;

        trace!("RAM store16 {:08x} => {:04x}", offset, val);

        let b0 = val >> 0;
        let b1 = val >> 8;

        self.data[offset + 0] = b0 as u8;
        self.data[offset + 1] = b1 as u8;
    }

    pub fn store8(&mut self, offset: u32, val: u8) {
        let offset = offset as usize;

        trace!("RAM store8 {:08x} => {:02x}", offset, val);

        self.data[offset] = val;
    }
}
