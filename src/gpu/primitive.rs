use std::mem::size_of;

use bytemuck::{Pod, Zeroable};

#[derive(Clone, Copy, Default, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: [f32; 3],
}

impl Vertex {
    pub fn new(pos: Position, col: Color) -> Self {
        Self {
            position: [pos.0 as f32, pos.1 as f32],
            color: [
                col.0 as f32 / 256.0,
                col.1 as f32 / 256.0,
                col.2 as f32 / 256.0,
            ],
        }
    }

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                },
            ],
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Position(pub i16, pub i16);

impl Position {
    pub fn from_gp0(val: u32) -> Position {
        let x = val as i16;
        let y = (val >> 16) as i16;

        Position(x, y)
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    pub fn from_gp0(val: u32) -> Color {
        let r = val as u8;
        let g = (val >> 8) as u8;
        let b = (val > 16) as u8;

        Color(r, g, b)
    }
}
