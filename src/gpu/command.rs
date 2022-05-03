pub struct CommandBuffer {
    buffer: [u32; 12],
    len: u8,
}

impl CommandBuffer {
    pub fn new() -> CommandBuffer {
        CommandBuffer {
            buffer: [0; 12],
            len: 0,
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn push_word(&mut self, word: u32) {
        self.buffer[self.len as usize] = word;

        self.len += 1;
    }

    pub fn val1(&self) -> u32 {
        self.buffer[0]
    }
}

impl std::ops::Index<usize> for CommandBuffer {
    type Output = u32;

    fn index(&self, index: usize) -> &u32 {
        if index >= self.len as usize {
            panic!(
                "Command buffer index out of range: {} ({})",
                index, self.len
            );
        }

        &self.buffer[index]
    }
}
