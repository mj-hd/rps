use std::collections::VecDeque;

use log::warn;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use vectrix::{Matrix, Vector};

use crate::{addressible::Addressible, cpu::RegisterIndex};

struct GteInstruction(u32);

#[derive(Debug, FromPrimitive)]
enum MultiplyMatrixType {
    Rotation = 0,
    Light = 1,
    Color = 2,
}

#[derive(Debug, FromPrimitive)]
enum MultiplyVectorType {
    V0 = 0,
    V1 = 1,
    V2 = 2,
    IrLong = 3,
}

#[derive(Debug, FromPrimitive)]
enum TranslationVectorType {
    Tr = 0,
    Bk = 1,
    FcBugged = 2,
    None = 3,
}

// 実際は25bit
impl GteInstruction {
    pub fn op_sf(self) -> bool {
        let GteInstruction(op) = self;

        op & 0x80000 > 0
    }

    pub fn op_mvmva_multiply_matrix(self) -> MultiplyMatrixType {
        let GteInstruction(op) = self;

        MultiplyMatrixType::from_u32((op >> 17) & 0b11).unwrap()
    }

    pub fn op_mvmva_multiply_vector(self) -> MultiplyVectorType {
        let GteInstruction(op) = self;

        MultiplyVectorType::from_u32((op >> 15) & 0b11).unwrap()
    }

    pub fn op_mvmva_translation_vector(self) -> TranslationVectorType {
        let GteInstruction(op) = self;

        TranslationVectorType::from_u32((op >> 13) & 0b11).unwrap()
    }

    pub fn op_saturate(self) -> bool {
        let GteInstruction(op) = self;

        op & 0x400 > 0
    }

    pub fn op_command(self) -> u32 {
        let GteInstruction(op) = self;

        op & 0x3F
    }
}

pub struct Gte {
    v0: Vector<i16, 3>,
    v1: Vector<i16, 3>,
    v2: Vector<i16, 3>,

    color: (u8, u8, u8, u8),

    otz: u16,
    ir0: i16,
    ir1: i16,
    ir2: i16,
    ir3: i16,

    sxy: VecDeque<(i16, i16)>,
    sz: VecDeque<(i16, i16)>,
    rgb: VecDeque<(u8, u8, u8, u8)>,

    mac0: i32,
    mac1: i32,
    mac2: i32,
    mac3: i32,

    irgb: u16,
    orgb: u16,
    lzcs: i32,
    lzcr: i32,

    rotation: Matrix<i16, 3, 3>,
    translation: Vector<i32, 3>,
    light_source: Matrix<i16, 3, 3>,
    background_color: (u32, u32, u32),
    light_color_source: Matrix<i32, 3, 3>,
    far_color: (u32, u32, u32),
    offset: (i32, i32),
    projection_distance: u16,
    depth_coeff: i16,
    depth_offset: u32,
    average_z_scale_3: i16,
    average_z_scale_4: i16,
    flag: u32,
}

impl Gte {
    pub fn new() -> Self {
        Gte {
            v0: Vector::zero(),
            v1: Vector::zero(),
            v2: Vector::zero(),
            color: (0, 0, 0, 0),
            otz: 0,
            ir0: 0,
            ir1: 0,
            ir2: 0,
            ir3: 0,
            sxy: VecDeque::new(),
            sz: VecDeque::new(),
            rgb: VecDeque::new(),
            mac0: 0,
            mac1: 0,
            mac2: 0,
            mac3: 0,
            irgb: 0,
            orgb: 0,
            lzcs: 0,
            lzcr: 0,
            rotation: Matrix::identity(),
            translation: Vector::zero(),
            light_source: Matrix::identity(),
            background_color: (0, 0, 0),
            light_color_source: Matrix::identity(),
            far_color: (0, 0, 0),
            offset: (0, 0),
            projection_distance: 0,
            depth_coeff: 0,
            depth_offset: 0,
            average_z_scale_3: 0,
            average_z_scale_4: 0,
            flag: 0,
        }
    }

    pub fn load_data<T: Addressible>(&self, offset: RegisterIndex) -> T {
        match offset.0 {
            24 => Addressible::from_u32(self.mac0 as u32),
            25 => Addressible::from_u32(self.mac1 as u32),
            26 => Addressible::from_u32(self.mac2 as u32),
            27 => Addressible::from_u32(self.mac3 as u32),
            28 => Addressible::from_u32(self.irgb as u32),
            29 => Addressible::from_u32(self.orgb as u32),
            30 => Addressible::from_u32(self.lzcs as u32),
            31 => Addressible::from_u32(self.lzcr as u32),
            _ => panic!("unhandled GTE DATA load offset: {:04x}", offset.0,),
        }
    }

    pub fn store_data<T: Addressible>(&mut self, offset: RegisterIndex, val: T) {
        match offset.0 {
            24 => {
                self.mac0 = val.as_u32() as i32;
            }
            25 => {
                self.mac1 = val.as_u32() as i32;
            }
            26 => {
                self.mac2 = val.as_u32() as i32;
            }
            27 => {
                self.mac3 = val.as_u32() as i32;
            }
            28 => {
                self.irgb = val.as_u32() as u16;
            }
            29 => {
                self.orgb = val.as_u32() as u16;
            }
            30 => {
                self.lzcs = val.as_u32() as i32;
            }
            31 => {
                self.lzcr = val.as_u32() as i32;
            }
            _ => panic!(
                "unhandled GTE DATA store offset: {:04x} val: {:04x}",
                offset.0,
                val.as_u32()
            ),
        }
    }

    pub fn load_control<T: Addressible>(&self, offset: RegisterIndex) -> T {
        warn!("unhandled GTE CONTROL load offset: {:04x}", offset.0);
        Addressible::from_u32(0)
    }

    pub fn store_control<T: Addressible>(&mut self, offset: RegisterIndex, val: T) {}

    pub fn command(&mut self, command: u32) {
        match command {
            _ => panic!("unhandled GTE instruction {:04x}", command),
        }
    }
}
