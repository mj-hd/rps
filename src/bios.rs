use anyhow::{bail, Result};
use log::trace;
use std::{fs::File, io::Read, path::Path};

const BIOS_SIZE: u64 = 512 * 1024;

pub struct Bios {
    data: Vec<u8>,
}

impl Bios {
    pub fn new(path: &Path) -> Result<Bios> {
        let file = File::open(path)?;

        let mut data = Vec::new();

        file.take(BIOS_SIZE).read_to_end(&mut data)?;

        if data.len() != BIOS_SIZE as usize {
            bail!("Invalid BIOS Size");
        }

        Ok(Bios { data })
    }

    pub fn load32(&self, offset: u32) -> u32 {
        let offset = offset as usize;

        let b0 = self.data[offset + 0] as u32;
        let b1 = self.data[offset + 1] as u32;
        let b2 = self.data[offset + 2] as u32;
        let b3 = self.data[offset + 3] as u32;

        let result = b0 | (b1 << 8) | (b2 << 16) | (b3 << 24);

        trace!("BIOS load32 {:08x} => {:08x}", offset, result);

        result
    }

    pub fn load8(&self, offset: u32) -> u8 {
        let offset = offset as usize;

        let result = self.data[offset];

        trace!("BIOS load8 {:08x} => {:02x}", offset, result);

        result
    }
}
