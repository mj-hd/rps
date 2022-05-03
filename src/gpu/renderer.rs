use std::ffi::CStr;
use std::fmt::Debug;
use std::{ffi::CString, ptr};
use std::{mem::size_of, slice};

use gl::types::{GLchar, GLsizei, GLsizeiptr};
use gl::{
    self,
    types::{GLenum, GLint, GLuint},
};
use log::warn;

use super::primitive::{Color, Position};

pub struct Renderer {
    window: sdl2::video::Window,
    gl_context: sdl2::video::GLContext,
    vertex_shader: GLuint,
    fragment_shader: GLuint,
    program: GLuint,
    vertex_array_object: GLuint,
    positions: Buffer<Position>,
    colors: Buffer<Color>,
    uniform_offset: GLint,
    nvertices: u32,
}

impl Renderer {
    pub fn new(sdl_context: &sdl2::Sdl) -> Renderer {
        let video_subsystem = sdl_context.video().unwrap();

        let gl_attr = video_subsystem.gl_attr();
        gl_attr.set_context_version(3, 3);
        gl_attr.set_double_buffer(true);
        gl_attr.set_depth_size(24);
        gl_attr.set_context_flags().debug().set();

        let window = video_subsystem
            .window("rps", 1024, 512)
            .opengl()
            .resizable()
            .build()
            .unwrap();

        let gl_context = window.gl_create_context().unwrap();
        gl::load_with(|name| video_subsystem.gl_get_proc_address(name) as *const _);

        let vs_src = include_str!("glsl/renderer.vert");
        let fs_src = include_str!("glsl/renderer.frag");

        let vertex_shader = compile_shader(vs_src, gl::VERTEX_SHADER);
        let fragment_shader = compile_shader(fs_src, gl::FRAGMENT_SHADER);

        let program = link_program(vertex_shader, fragment_shader);

        let mut vao = 0;
        unsafe {
            gl::GenVertexArrays(1, &mut vao);
            gl::BindVertexArray(vao);
        }

        unsafe {
            gl::UseProgram(program);
        }

        let positions = Buffer::<Position>::new();

        unsafe {
            let index = find_program_attrib(program, "vertex_position");
            gl::EnableVertexAttribArray(index);
            gl::VertexAttribIPointer(index, 2, gl::SHORT, 0, ptr::null());
        }

        let colors = Buffer::<Color>::new();

        unsafe {
            let index = find_program_attrib(program, "vertex_color");
            gl::EnableVertexAttribArray(index);
            gl::VertexAttribIPointer(index, 3, gl::UNSIGNED_BYTE, 0, ptr::null());
        }

        let uniform_offset = find_program_uniform(program, "offset");

        unsafe {
            gl::Uniform2i(uniform_offset, 0, 0);
        }

        unsafe {
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

        window.gl_swap_window();

        Renderer {
            window,
            gl_context,
            vertex_shader,
            fragment_shader,
            program,
            vertex_array_object: vao,
            positions,
            colors,
            uniform_offset,
            nvertices: 0,
        }
    }

    pub fn push_triangles(&mut self, positions: [Position; 3], colors: [Color; 3]) {
        if self.nvertices + 3 > VERTEX_BUFFER_LEN {
            println!("Vertex attribute buffers full, forcing draw");
            self.draw();
        }

        for i in 0..3 {
            self.positions.set(self.nvertices, positions[i]);
            self.colors.set(self.nvertices, colors[i]);
            self.nvertices += 1;
        }
    }

    pub fn push_quad(&mut self, positions: [Position; 4], colors: [Color; 4]) {
        if self.nvertices + 6 > VERTEX_BUFFER_LEN {
            println!("Vertex attribute buffers full, forcing draw");
            self.draw();
        }

        for i in 0..3 {
            self.positions.set(self.nvertices, positions[i]);
            self.colors.set(self.nvertices, colors[i]);
            self.nvertices += 1;
        }

        for i in 1..4 {
            self.positions.set(self.nvertices, positions[i]);
            self.colors.set(self.nvertices, colors[i]);
            self.nvertices += 1;
        }
    }

    pub fn set_draw_offset(&mut self, x: i16, y: i16) {
        self.draw();

        unsafe {
            gl::Uniform2i(self.uniform_offset, x as GLint, y as GLint);
        }
    }

    pub fn draw(&mut self) {
        unsafe {
            gl::MemoryBarrier(gl::CLIENT_MAPPED_BUFFER_BARRIER_BIT);
            gl::DrawArrays(gl::TRIANGLES, 0, self.nvertices as GLsizei);
        }

        unsafe {
            let sync = gl::FenceSync(gl::SYNC_GPU_COMMANDS_COMPLETE, 0);

            loop {
                let r = gl::ClientWaitSync(sync, gl::SYNC_FLUSH_COMMANDS_BIT, 10000000);

                if r == gl::ALREADY_SIGNALED || r == gl::CONDITION_SATISFIED {
                    break;
                }
            }
        }

        self.nvertices = 0;
    }

    pub fn display(&mut self) {
        self.draw();

        self.window.gl_swap_window();

        check_for_errors();
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &self.vertex_array_object);
            gl::DeleteShader(self.vertex_shader);
            gl::DeleteShader(self.fragment_shader);
            gl::DeleteProgram(self.program);
        }
    }
}

pub fn check_for_errors() {
    let mut fatal = false;

    loop {
        let mut buffer = vec![0; 4096];
        let mut severity = 0;
        let mut source = 0;
        let mut messaage_size = 0;
        let mut mtype = 0;
        let mut id = 0;

        let count = unsafe {
            gl::GetDebugMessageLog(
                1,
                buffer.len() as GLsizei,
                &mut source,
                &mut mtype,
                &mut id,
                &mut severity,
                &mut messaage_size,
                buffer.as_mut_ptr() as *mut GLchar,
            )
        };

        if count == 0 {
            break;
        }

        buffer.truncate(messaage_size as usize);

        let message = match std::str::from_utf8(&buffer) {
            Ok(m) => m,
            Err(e) => panic!("Got invalid message: {}", e),
        };

        let source = DebugSource(source);
        let severity = DebugSeverity(severity);
        let mtype = DebugType(mtype);

        warn!(
            "OpenGL [{:?}|{:?}|{:?}|0x{:x}] {}",
            severity, source, mtype, id, message
        );

        if severity.is_fatal() {
            fatal = true;
        }
    }

    if fatal {
        panic!("Fatal OpenGL error");
    }
}

struct DebugSource(GLenum);

impl Debug for DebugSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            gl::DEBUG_SOURCE_API => f.write_str("api"),
            gl::DEBUG_SOURCE_OTHER => f.write_str("other"),
            gl::DEBUG_SOURCE_THIRD_PARTY => f.write_str("third_party"),
            gl::DEBUG_SOURCE_APPLICATION => f.write_str("application"),
            gl::DEBUG_SOURCE_WINDOW_SYSTEM => f.write_str("window_system"),
            gl::DEBUG_SOURCE_SHADER_COMPILER => f.write_str("shader_compiler"),
            _ => f.write_str(&self.0.to_string()),
        }
    }
}

struct DebugSeverity(GLenum);

impl DebugSeverity {
    fn is_fatal(&self) -> bool {
        match self.0 {
            gl::DEBUG_SEVERITY_HIGH => true,
            _ => false,
        }
    }
}

impl Debug for DebugSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            gl::DEBUG_SEVERITY_LOW => f.write_str("low"),
            gl::DEBUG_SEVERITY_MEDIUM => f.write_str("medium"),
            gl::DEBUG_SEVERITY_HIGH => f.write_str("high"),
            gl::DEBUG_SEVERITY_NOTIFICATION => f.write_str("notification"),
            _ => f.write_str(&self.0.to_string()),
        }
    }
}

struct DebugType(GLenum);

impl Debug for DebugType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            gl::DEBUG_TYPE_ERROR => f.write_str("error"),
            gl::DEBUG_TYPE_OTHER => f.write_str("other"),
            gl::DEBUG_TYPE_MARKER => f.write_str("marker"),
            gl::DEBUG_TYPE_POP_GROUP => f.write_str("pop_group"),
            gl::DEBUG_TYPE_PUSH_GROUP => f.write_str("push_group"),
            gl::DEBUG_TYPE_PORTABILITY => f.write_str("portability"),
            gl::DEBUG_TYPE_PERFORMANCE => f.write_str("performance"),
            gl::DEBUG_TYPE_UNDEFINED_BEHAVIOR => f.write_str("undefined_behavior"),
            gl::DEBUG_TYPE_DEPRECATED_BEHAVIOR => f.write_str("deprecated_behavior"),
            _ => f.write_str(&self.0.to_string()),
        }
    }
}

fn compile_shader(src: &str, shader_type: GLenum) -> GLuint {
    let shader;

    unsafe {
        shader = gl::CreateShader(shader_type);

        let c_str = CString::new(src.as_bytes()).unwrap();
        gl::ShaderSource(shader, 1, &c_str.as_ptr(), ptr::null());
        gl::CompileShader(shader);

        let mut status = gl::FALSE as GLint;
        gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut status);

        if status != (gl::TRUE as GLint) {
            let mut length = 0 as GLint;
            gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut length);
            let buf = vec![0 as GLchar; length as usize].as_mut_ptr() as *mut GLchar;
            gl::GetShaderInfoLog(shader, length, ptr::null_mut(), buf);
            panic!("Shader compilation failed! {:?}", CStr::from_ptr(buf));
        }
    }

    shader
}

fn link_program(vs: GLuint, fs: GLuint) -> GLuint {
    let program;

    unsafe {
        program = gl::CreateProgram();

        gl::AttachShader(program, vs);
        gl::AttachShader(program, fs);

        gl::LinkProgram(program);

        let mut status = gl::FALSE as GLint;

        gl::GetProgramiv(program, gl::LINK_STATUS, &mut status);

        if status != (gl::TRUE as GLint) {
            let mut length = 0 as GLint;
            gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut length);
            let buf = vec![0 as GLchar; length as usize].as_mut_ptr() as *mut GLchar;
            gl::GetProgramInfoLog(program, length, ptr::null_mut(), buf);
            panic!("OpenGL program linking failed! {:?}", CStr::from_ptr(buf));
        }
    }

    program
}

fn find_program_attrib(program: GLuint, attr: &str) -> GLuint {
    let cstr = CString::new(attr).unwrap().into_raw();

    let index = unsafe { gl::GetAttribLocation(program, cstr) };

    if index < 0 {
        panic!("Attribute \"{}\" not found in program ({})", attr, index);
    }

    index as GLuint
}

fn find_program_uniform(program: GLuint, uniform: &str) -> GLint {
    let cstr = CString::new(uniform).unwrap().into_raw();

    let index = unsafe { gl::GetUniformLocation(program, cstr) };

    if index < 0 {
        panic!("Uniform \"{}\" not found in program ({})", uniform, index);
    }

    index as GLint
}

pub struct Buffer<T> {
    object: GLuint,
    map: *mut T,
}

impl<T: Copy + Default> Buffer<T> {
    pub fn new() -> Buffer<T> {
        let mut object = 0;
        let memory;

        unsafe {
            gl::GenBuffers(1, &mut object);
            gl::BindBuffer(gl::ARRAY_BUFFER, object);

            let element_size = size_of::<T>() as GLsizeiptr;
            let buffer_size = element_size * VERTEX_BUFFER_LEN as GLsizeiptr;

            gl::BufferStorage(
                gl::ARRAY_BUFFER,
                buffer_size,
                ptr::null(),
                gl::MAP_WRITE_BIT | gl::MAP_PERSISTENT_BIT,
            );

            memory =
                gl::MapBufferRange(gl::ARRAY_BUFFER, 0, buffer_size, gl::MAP_WRITE_BIT) as *mut T;

            if (memory as u32) == 0 {
                panic!(
                    "failed to map array buffer, element_size: {}, buffer_size: {}, err: {:?}",
                    element_size,
                    buffer_size,
                    gl::GetError()
                );
            }

            let s = slice::from_raw_parts_mut(memory, VERTEX_BUFFER_LEN as usize);

            for x in s.iter_mut() {
                *x = Default::default();
            }
        }

        Buffer {
            object,
            map: memory,
        }
    }

    pub fn set(&mut self, index: u32, val: T) {
        if index >= VERTEX_BUFFER_LEN {
            panic!("buffer overflow!");
        }

        unsafe {
            let p = self.map.offset(index as isize);

            *p = val;
        }
    }
}

impl<T> Drop for Buffer<T> {
    fn drop(&mut self) {
        unsafe {
            gl::BindBuffer(gl::ARRAY_BUFFER, self.object);
            gl::UnmapBuffer(gl::ARRAY_BUFFER);
            gl::DeleteBuffers(1, &self.object);
        }
    }
}

const VERTEX_BUFFER_LEN: u32 = 64 * 1024;
