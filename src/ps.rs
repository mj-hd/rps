use crate::{cpu::cpu::Cpu, interconnect::Interconnect};

pub struct Ps {
    cpu: Cpu,
    interconnect: Interconnect,
}

impl Ps {
    pub fn new(cpu: Cpu, interconnect: Interconnect) -> Self {
        Self { cpu, interconnect }
    }

    pub async fn run() {

    }
}
