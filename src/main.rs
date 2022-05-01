use std::path::Path;

use rps::{bios::Bios, cpu::cpu::Cpu, interconnect::Interconnect};

fn main() {
    env_logger::init();

    let bios = Bios::new(&Path::new("roms/bios.rom")).unwrap();

    let inter = Interconnect::new(bios);

    let mut cpu = Cpu::new(inter);

    loop {
        cpu.run_next_instruction();
    }
}
