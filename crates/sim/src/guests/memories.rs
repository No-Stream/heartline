//! Compact guest memories of completed rides.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MemoryEntry {
    pub ride_id: u32,
    pub enjoyment: i8,
    pub nausea: u8,
    pub tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GuestMemory {
    entries: [MemoryEntry; Self::CAPACITY],
    start: u8,
    len: u8,
}

impl GuestMemory {
    pub const CAPACITY: usize = 8;

    pub fn push(&mut self, entry: MemoryEntry) {
        let index = (usize::from(self.start) + usize::from(self.len)) % Self::CAPACITY;
        if usize::from(self.len) == Self::CAPACITY {
            self.entries[usize::from(self.start)] = entry;
            self.start = ((usize::from(self.start) + 1) % Self::CAPACITY) as u8;
        } else {
            self.entries[index] = entry;
            self.len += 1;
        }
    }

    pub fn len(&self) -> usize {
        usize::from(self.len)
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = &MemoryEntry> {
        (0..self.len())
            .map(|offset| &self.entries[(usize::from(self.start) + offset) % Self::CAPACITY])
    }
}

impl Default for GuestMemory {
    fn default() -> Self {
        Self {
            entries: [MemoryEntry::default(); Self::CAPACITY],
            start: 0,
            len: 0,
        }
    }
}
