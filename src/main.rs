use std::path::Path;

use rps::{
    bios::Bios,
    cpu::cpu::Cpu,
    gpu::{gpu::Gpu, renderer::Renderer},
    interconnect::Interconnect,
};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    pollster::block_on(run());
}

async fn run() {
    env_logger::init();

    let event_loop = EventLoop::new();
    let size = LogicalSize::<u32>::new(1024, 512);
    let window = WindowBuilder::new()
        .with_title("rps")
        .with_inner_size(size)
        .with_min_inner_size(size)
        .build(&event_loop)
        .unwrap();

    let bios = Bios::new(&Path::new("roms/scph5500.rom")).unwrap();

    let renderer = Renderer::new(&window).await;
    let gpu = Gpu::new(renderer);
    let inter = Interconnect::new(bios, gpu);

    let mut cpu = Cpu::new(inter);

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => *control_flow = ControlFlow::Exit,
        Event::MainEventsCleared => {
            for _ in 0..1_000_000 {
                cpu.run_next_instruction();
            }
        }
        _ => {
            *control_flow = ControlFlow::Poll;
        }
    });
}
