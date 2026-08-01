pub fn fnv1a(data: &[u8]) -> u32 {
    let mut hash = 0x811c9dc5;

    for &b in data {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }

    hash
}
