use std::path::Path;

use rps::interconnect::Interconnect;

fn main() {
    let bios = Bios::new(&Path::new("roms/bios.rom")).unwrap();

    let inter = Interconnect::new(bios);

    let mut cpu = Cpu::new(inter);

    loop {
        cpu.run_next_instruction();
    }
}
