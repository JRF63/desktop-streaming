mod cache_aligned;
mod cyclic_buffer;
mod reader;
mod writer;

use std::sync::Arc;
use cyclic_buffer::CyclicBuffer;
use reader::Reader;
use writer::Writer;

pub fn channel<T, const N: usize>(buffer: [T; N]) -> Option<(Writer<T, N>, Reader<T, N>,)> {
    let writer_buffer = Arc::new(CyclicBuffer::new(buffer)?);
    let reader_buffer = writer_buffer.clone();

    let writer = Writer::new(writer_buffer);
    let reader = Reader::new(reader_buffer);
    Some((writer, reader))
}

#[cfg(test)]
mod tests {
    #[test]
    fn channel_sanity_check() {
        const N: i32 = 100;

        let buf = [0; 8];
        let (writer, reader) = super::channel(buf).unwrap();

        let tw = std::thread::spawn(move || {
            let writer = writer;
            for i in 0..N {
                writer.write((), |_i, val, ()| {
                    *val = i;
                });
            }
        });

        let tr = std::thread::spawn(move || {
            let reader = reader;
            for i in 0..N {
                reader.read(|val| {
                    assert_eq!(i, *val);
                }); 
            }
        });

        tw.join().unwrap();
        tr.join().unwrap();
    }
}