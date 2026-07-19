pub const PAGE_SIZE: usize = 8192;

#[derive(Clone)]
pub struct RawPage {
    pub data: [u8; PAGE_SIZE],
}

impl RawPage {
    pub fn new() -> Self {
        Self {
            data: [0u8; PAGE_SIZE],
        }
    }

    pub fn from_bytes(data: [u8; PAGE_SIZE]) -> Self {
        Self { data }
    }

    pub fn as_bytes(&self) -> &[u8; PAGE_SIZE] {
        &self.data
    }

    pub fn as_bytes_mut(&mut self) -> &mut [u8; PAGE_SIZE] {
        &mut self.data
    }
}

impl AsRef<[u8; PAGE_SIZE]> for RawPage {
    fn as_ref(&self) -> &[u8; PAGE_SIZE] {
        &self.data
    }
}

impl AsMut<[u8; PAGE_SIZE]> for RawPage {
    fn as_mut(&mut self) -> &mut [u8; PAGE_SIZE] {
        &mut self.data
    }
}

impl AsRef<[u8]> for RawPage {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl AsMut<[u8]> for RawPage {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}
