use std::{
    net::{TcpListener, TcpStream},
    path::Path,
    sync::mpsc,
    thread,
};

use clap::{Arg, Command};
use gdbstub::{
    common::Signal,
    conn::{Connection, ConnectionExt},
    stub::{run_blocking, DisconnectReason, GdbStub, GdbStubError, SingleThreadStopReason},
    target::Target,
};
use rps::{
    bios::Bios,
    cpu::{cpu, cpu::Cpu},
    gpu::{gpu::Gpu, renderer::Renderer},
    interconnect::Interconnect,
};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

type DynResult<T> = Result<T, Box<dyn std::error::Error>>;

enum PsThreadEvent {}

enum UiThreadEvent {}

fn main() {
    pollster::block_on(run()).unwrap();
}

async fn run() -> DynResult<()> {
    env_logger::init();

    let matches = Command::new("rps")
        .about("PlayStation Emulator")
        .version("0.1.0")
        .author("mjhd <mjhd.devlion@gmail.com>")
        .arg(
            Arg::new("debug")
                .short('d')
                .long("debug")
                .help("enable gdb remote debugging"),
        )
        .get_matches();

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

    let (ps_sender, ps_receiver) = mpsc::sync_channel::<PsThreadEvent>(1);
    let (ui_sender, ui_receiver) = mpsc::sync_channel::<UiThreadEvent>(1);

    {
        let mut cpu = Cpu::new(inter);

        thread::spawn(move || {
            if !matches.is_present("debug") {
                while cpu.step() != Some(cpu::Event::Halted) {}

                return;
            }

            let connection: Box<dyn ConnectionExt<Error = std::io::Error>> =
                Box::new(wait_for_tcp(9001).unwrap());
            let gdb = GdbStub::new(connection);
            match gdb.run_blocking::<EmuGdbEventLoop>(&mut cpu) {
                Ok(disconnect_reason) => match disconnect_reason {
                    DisconnectReason::Disconnect => {
                        println!("GDB client has disconnected. Running to completion...");
                        while cpu.step() != Some(cpu::Event::Halted) {}
                    }
                    DisconnectReason::TargetExited(code) => {
                        println!("Target exited with code {}!", code)
                    }
                    DisconnectReason::TargetTerminated(sig) => {
                        println!("Target terminated with signal {}!", sig)
                    }
                    DisconnectReason::Kill => println!("GDB sent a kill command!"),
                },
                Err(GdbStubError::TargetError(e)) => {
                    println!("target encountered a fatal error: {}", e)
                }
                Err(e) => {
                    println!("gdbstub encountered a fatal error: {}", e)
                }
            };
        });
    }

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => *control_flow = ControlFlow::Exit,
        _ => {
            *control_flow = ControlFlow::Poll;
        }
    });
}

fn wait_for_tcp(port: u16) -> DynResult<TcpStream> {
    let sockaddr = format!("127.0.0.1:{}", port);
    eprintln!("Waiting for a GDB connection on {:?}...", sockaddr);

    let sock = TcpListener::bind(sockaddr)?;
    let (stream, addr) = sock.accept()?;
    eprintln!("Debugger connected from {}", addr);

    Ok(stream)
}

enum EmuGdbEventLoop {}

impl run_blocking::BlockingEventLoop for EmuGdbEventLoop {
    type Target = Cpu;
    type Connection = Box<dyn ConnectionExt<Error = std::io::Error>>;
    type StopReason = SingleThreadStopReason<u32>;

    #[allow(clippy::type_complexity)]
    fn wait_for_stop_reason(
        target: &mut Cpu,
        conn: &mut Self::Connection,
    ) -> Result<
        run_blocking::Event<SingleThreadStopReason<u32>>,
        run_blocking::WaitForStopReasonError<
            <Self::Target as Target>::Error,
            <Self::Connection as Connection>::Error,
        >,
    > {
        let poll_incoming_data = || conn.peek().map(|b| b.is_some()).unwrap_or(true);

        match target.run(poll_incoming_data) {
            cpu::RunEvent::IncomingData => {
                let byte = conn
                    .read()
                    .map_err(run_blocking::WaitForStopReasonError::Connection)?;
                Ok(run_blocking::Event::IncomingData(byte))
            }
            cpu::RunEvent::Event(event) => {
                use gdbstub::target::ext::breakpoints::WatchKind;

                let stop_reason = match event {
                    cpu::Event::DoneStep => SingleThreadStopReason::DoneStep,
                    cpu::Event::Halted => SingleThreadStopReason::Terminated(Signal::SIGSTOP),
                    cpu::Event::Break => SingleThreadStopReason::SwBreak(()),
                    cpu::Event::WatchWrite(addr) => SingleThreadStopReason::Watch {
                        tid: (),
                        kind: WatchKind::Write,
                        addr,
                    },
                    cpu::Event::WatchRead(addr) => SingleThreadStopReason::Watch {
                        tid: (),
                        kind: WatchKind::Read,
                        addr,
                    },
                };

                Ok(run_blocking::Event::TargetStopped(stop_reason))
            }
        }
    }

    fn on_interrupt(
        _target: &mut Cpu,
    ) -> Result<Option<SingleThreadStopReason<u32>>, <Cpu as Target>::Error> {
        Ok(Some(SingleThreadStopReason::Signal(Signal::SIGINT)))
    }
}
