pub trait ExtendFromBytes {
    fn extend_from_bytes(&mut self, bytes: &[u8]);
}

impl ExtendFromBytes for Vec<i16> {
    fn extend_from_bytes(&mut self, bytes: &[u8]) {
        for pair in bytes.chunks_exact(2) {
            if let &[a, b] = pair {
                self.push(i16::from_le_bytes([a, b]))
            }
        }
    }
}