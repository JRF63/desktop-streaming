mod codec;
mod format;
mod status;

pub(crate) use codec::MediaCodec;
pub(crate) use format::VideoType;

pub(crate) fn aspect_ratio_string(width: i32, height: i32) -> String {
    //https://en.wikipedia.org/wiki/Binary_GCD_algorithm
    pub fn gcd(mut u: i32, mut v: i32) -> i32 {
        use std::cmp::min;
        use std::mem::swap;

        if u == 0 {
            return v;
        } else if v == 0 {
            return u;
        }

        let i = u.trailing_zeros();
        u >>= i;
        let j = v.trailing_zeros();
        v >>= j;
        let k = min(i, j);

        loop {
            if u > v {
                swap(&mut u, &mut v);
            }
            v -= u;
            if v == 0 {
                return u << k;
            }
            v >>= v.trailing_zeros();
        }
    }
    let divisor = gcd(width, height);
    format!("{}:{}", width / divisor, height / divisor)
}

#[cfg(test)]
mod tests {
    #[test]
    fn aspect_ratio_string() {
        let width = 1920;
        let height = 1080;
        assert_eq!("16:9", super::aspect_ratio_string(width, height));
    }
}