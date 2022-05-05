use anyhow::{bail, Result};
use log::trace;
use std::{fs::File, io::Read, path::Path};

use crate::addressible::Addressible;

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

    pub fn load<T: Addressible>(&self, offset: u32) -> T {
        let offset = offset as usize;

        let mut v = 0;

        for i in 0..T::width() as usize {
            v |= (self.data[offset + i] as u32) << (i * 8);
        }

        trace!("BIOS{:?} load {:08x} => {:08x}", T::width(), offset, v);

        Addressible::from_u32(v)
    }
}
