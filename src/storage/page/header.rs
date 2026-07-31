pub const HEADER_SIZE: usize = 24;
pub const SLOT_SIZE: usize = 4;

#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PageType {
    Heap = 0,
    Index = 1,
    Overflow = 2,
    FreeSpaceMap = 3,
    VisibilityMap = 4,
}

impl TryFrom<u16> for PageType {
    type Error = String;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(PageType::Heap),
            1 => Ok(PageType::Index),
            2 => Ok(PageType::Overflow),
            3 => Ok(PageType::FreeSpaceMap),
            4 => Ok(PageType::VisibilityMap),
            other => Err(format!("Invalid page type discriminant: {}", other)),
        }
    }
}

pub struct PageFlags;
impl PageFlags {
    pub const DIRTY: u16 = 1 << 0;
    pub const PINNED: u16 = 1 << 1;
    pub const COMPACT: u16 = 1 << 2;
    pub const HAS_TOAST: u16 = 1 << 3;

    pub fn is_set(flags: u16, flag: u16) -> bool {
        (flags & flag) != 0
    }
}

/// Represents the deserialized 24-byte page header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageHeader {
    pub page_id: u32,
    pub page_lsn: u64,
    pub checksum: u32,
    pub slot_count: u16,
    pub free_space_pointer: u16,
    pub page_type: PageType,
    pub flags: u16,
}

impl PageHeader {
    /// Deserializes a 24-byte header slice.
    pub fn from_bytes(bytes: &[u8; HEADER_SIZE]) -> Result<Self, String> {
        let page_id = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let page_lsn = u64::from_le_bytes(bytes[4..12].try_into().unwrap());
        let checksum = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        let slot_count = u16::from_le_bytes(bytes[16..18].try_into().unwrap());
        let free_space_pointer = u16::from_le_bytes(bytes[18..20].try_into().unwrap());
        let page_type_raw = u16::from_le_bytes(bytes[20..22].try_into().unwrap());
        let flags = u16::from_le_bytes(bytes[22..24].try_into().unwrap());

        let page_type = PageType::try_from(page_type_raw)?;

        Ok(Self {
            page_id,
            page_lsn,
            checksum,
            slot_count,
            free_space_pointer,
            page_type,
            flags,
        })
    }

    /// Serializes header into a 24-byte array.
    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut buf = [0u8; HEADER_SIZE];
        buf[0..4].copy_from_slice(&self.page_id.to_le_bytes());
        buf[4..12].copy_from_slice(&self.page_lsn.to_le_bytes());
        buf[12..16].copy_from_slice(&self.checksum.to_le_bytes());
        buf[16..18].copy_from_slice(&self.slot_count.to_le_bytes());
        buf[18..20].copy_from_slice(&self.free_space_pointer.to_le_bytes());
        buf[20..22].copy_from_slice(&(self.page_type as u16).to_le_bytes());
        buf[22..24].copy_from_slice(&self.flags.to_le_bytes());
        buf
    }
}
