use gl::types::{GLshort, GLubyte};

#[derive(Clone, Copy, Default, Debug)]
pub struct Position(pub GLshort, pub GLshort);

impl Position {
    pub fn from_gp0(val: u32) -> Position {
        let x = val as i16;
        let y = (val >> 16) as i16;

        Position(x as GLshort, y as GLshort)
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Color(pub GLubyte, pub GLubyte, pub GLubyte);

impl Color {
    pub fn from_gp0(val: u32) -> Color {
        let r = val as u8;
        let g = (val >> 8) as u8;
        let b = (val > 16) as u8;

        Color(r as GLubyte, g as GLubyte, b as GLubyte)
    }
}
