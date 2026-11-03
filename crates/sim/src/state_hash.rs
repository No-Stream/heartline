/// Fixed FNV-1a writer for deterministic in-memory state fingerprints.
pub(crate) struct StateHasher {
    value: u64,
}

impl StateHasher {
    pub(crate) fn new() -> Self {
        Self {
            value: 0xcbf29ce484222325,
        }
    }

    pub(crate) fn finish(self) -> u64 {
        self.value
    }

    pub(crate) fn write_bytes(&mut self, bytes: &[u8]) {
        self.write_len(bytes.len());
        for &byte in bytes {
            self.value = (self.value ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }

    pub(crate) fn write_tag(&mut self, tag: &[u8]) {
        self.write_bytes(tag);
    }

    pub(crate) fn write_len(&mut self, value: usize) {
        self.write_u64(value as u64);
    }

    pub(crate) fn write_bool(&mut self, value: bool) {
        self.write_u8(u8::from(value));
    }

    pub(crate) fn write_u8(&mut self, value: u8) {
        self.write_raw(&[value]);
    }

    pub(crate) fn write_u16(&mut self, value: u16) {
        self.write_raw(&value.to_le_bytes());
    }

    pub(crate) fn write_u32(&mut self, value: u32) {
        self.write_raw(&value.to_le_bytes());
    }

    pub(crate) fn write_u64(&mut self, value: u64) {
        self.write_raw(&value.to_le_bytes());
    }

    pub(crate) fn write_i8(&mut self, value: i8) {
        self.write_raw(&value.to_le_bytes());
    }

    pub(crate) fn write_i16(&mut self, value: i16) {
        self.write_raw(&value.to_le_bytes());
    }

    pub(crate) fn write_i32(&mut self, value: i32) {
        self.write_raw(&value.to_le_bytes());
    }

    pub(crate) fn write_f32(&mut self, value: f32) {
        self.write_u32(value.to_bits());
    }

    pub(crate) fn write_f64(&mut self, value: f64) {
        self.write_u64(value.to_bits());
    }

    pub(crate) fn write_option<T>(&mut self, value: Option<T>, write: impl FnOnce(&mut Self, T)) {
        match value {
            Some(value) => {
                self.write_u8(1);
                write(self, value);
            }
            None => self.write_u8(0),
        }
    }

    fn write_raw(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.value = (self.value ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
}
