pub struct UnsafeBufMut<'a> {
    buffer: &'a mut [u8],
    index: usize,
}

impl<'a> UnsafeBufMut<'a> {
    #[inline(always)]
    pub fn new(buffer: &'a mut [u8]) -> UnsafeBufMut<'a> {
        UnsafeBufMut { buffer, index: 0 }
    }

    // Same as `bytes::BufMut` but without length checks.
    #[inline(always)]
    pub unsafe fn put_slice(&mut self, src: &[u8]) {
        let num_bytes = src.len();
        std::ptr::copy_nonoverlapping(
            src.as_ptr(),
            self.buffer.get_unchecked_mut(self.index..).as_mut_ptr(),
            num_bytes,
        );
        self.index = self.index.wrapping_add(num_bytes);
    }

    // Same as `bytes::BufMut` but directly inserts to the slice without checks.
    #[inline(always)]
    pub unsafe fn put_u8(&mut self, n: u8) {
        *self.buffer.get_unchecked_mut(self.index) = n;
        self.index += 1;
    }

    #[inline(always)]
    pub unsafe fn put_u16(&mut self, n: u16) {
        self.put_slice(&n.to_be_bytes());
    }

    #[inline(always)]
    pub fn remaining_mut(&self) -> usize {
        self.buffer.len() - self.index
    }

    #[inline(always)]
    pub fn num_bytes_written(&self) -> usize {
        self.index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_buf_mut() {
        let mut vec = vec![0u8; 8];
        let mut b = UnsafeBufMut::new(&mut vec);
        let data = [42, 42];
        unsafe {
            assert_eq!(b.remaining_mut(), 8);
            b.put_u8(42);
            assert_eq!(b.remaining_mut(), 7);
            assert_eq!(b.index, 1);
            b.put_slice(&data);
            assert_eq!(b.remaining_mut(), 5);
            assert_eq!(b.index, 3);
            b.put_u8(42);
            assert_eq!(b.remaining_mut(), 4);
            assert_eq!(b.index, 4);
        }
    }
}
