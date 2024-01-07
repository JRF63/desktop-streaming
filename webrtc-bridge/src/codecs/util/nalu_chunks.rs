/// Iterator over NALUs. The returned `&[u8]`s does not include the NALU delimiter.
pub struct NaluChunks<'a> {
    data: &'a [u8],
    start: usize,
}

impl<'a> Iterator for NaluChunks<'a> {
    type Item = &'a [u8];

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.start == self.data.len() {
            None
        } else {
            let (end, next_start) = next_ind(self.data, self.start);
            // SAFETY:
            // `next_ind` always returns valid indices
            let slice = unsafe { self.data.get_unchecked(self.start..end) };
            self.start = next_start;
            Some(slice)
        }
    }
}

#[inline]
fn next_ind(data: &[u8], start: usize) -> (usize, usize) {
    let mut zero_count = 0;

    for (i, &b) in data.iter().enumerate().skip(start) {
        if b == 0 {
            zero_count += 1;
            continue;
        } else if b == 1 && zero_count >= 2 {
            return (i - zero_count, i + 1);
        }
        zero_count = 0
    }
    (data.len(), data.len())
}

/// Returns an iterator over the NALU bytes of `data`.
#[inline]
pub fn nalu_chunks(data: &[u8]) -> NaluChunks {
    let (_, start) = next_ind(data, 0);
    NaluChunks { data, start }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nalu_chunks_test() {
        let tests: Vec<(&[u8], Option<&[u8]>)> = vec![
            (&[], None),
            (&[0, 0, 0, 1], None),
            (&[0, 0, 0, 1, 0, 0, 1], Some(&[])),
            (&[0, 0, 0, 1, 2], Some(&[2])),
            (&[0, 0, 0, 0], None),
        ];
        for (data, res) in tests {
            assert_eq!(nalu_chunks(&data).next(), res);
        }
    }
}
