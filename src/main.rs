use std::path::Path;

use rps::{
    bios::Bios,
    cpu::cpu::Cpu,
    gpu::{gpu::Gpu, renderer::Renderer},
    interconnect::Interconnect,
};
use sdl2::{event::Event, keyboard::Keycode};

fn main() {
    env_logger::init();

    let bios = Bios::new(&Path::new("roms/bios.rom")).unwrap();

    let sdl_context = sdl2::init().unwrap();

    let renderer = Renderer::new(&sdl_context);
    let gpu = Gpu::new(renderer);
    let inter = Interconnect::new(bios, gpu);

    let mut cpu = Cpu::new(inter);

    let mut event_pump = sdl_context.event_pump().unwrap();

    loop {
        for _ in 0..1_000_000 {
            cpu.run_next_instruction();
        }

        for e in event_pump.poll_iter() {
            match e {
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => return,
                Event::Quit { .. } => return,
                _ => (),
            }
        }
    }
}
