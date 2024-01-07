/// H.264 codec profile
#[non_exhaustive]
pub enum H264Profile {
    ConstrainedBaseline,
    Baseline,
    Main,
    Extended,
    High,
    ProgressiveHigh,
    ConstrainedHigh,
    High10,
    High422,
    High444,
    High10Intra,
    High422Intra,
    High444Intra,
    Cavlc444Intra,
    StereoHigh,
}

impl H264Profile {
    const IDC_BASELINE: u8 = 0x42;
    const IDC_MAIN: u8 = 0x4D;
    const IDC_EXTENDED: u8 = 0x58;
    const IDC_HIGH: u8 = 0x64;
    const IDC_HIGH_10: u8 = 0x6E;
    const IDC_HIGH_422: u8 = 0x7A;
    const IDC_HIGH_444: u8 = 0xF4;
    const IDC_CAVLC_444: u8 = 0x44;
    const IDC_STEREO_HIGH: u8 = 0x80;

    /// Parse the `H264Profile` as a partial profile-level-id.
    pub fn profile_idc_iop(self) -> String {
        // https://developer.mozilla.org/en-US/docs/Web/Media/Formats/codecs_parameter
        let (profile_idc, profile_iop): (u8, u8) = match self {
            H264Profile::ConstrainedBaseline => (H264Profile::IDC_BASELINE, 0xE0),
            H264Profile::Baseline => (H264Profile::IDC_BASELINE, 0),
            H264Profile::Main => (H264Profile::IDC_MAIN, 0),
            H264Profile::Extended => (H264Profile::IDC_EXTENDED, 0),
            H264Profile::High => (H264Profile::IDC_HIGH, 0),
            H264Profile::ProgressiveHigh => (H264Profile::IDC_HIGH, 0x08),
            H264Profile::ConstrainedHigh => (H264Profile::IDC_HIGH, 0x0C),
            H264Profile::High10 => (H264Profile::IDC_HIGH_10, 0),
            H264Profile::High422 => (H264Profile::IDC_HIGH_422, 0),
            H264Profile::High444 => (H264Profile::IDC_HIGH_444, 0),
            H264Profile::High10Intra => (H264Profile::IDC_HIGH_10, 0x10),
            H264Profile::High422Intra => (H264Profile::IDC_HIGH_422, 0x10),
            H264Profile::High444Intra => (H264Profile::IDC_HIGH_444, 0x10),
            H264Profile::Cavlc444Intra => (H264Profile::IDC_CAVLC_444, 0),
            H264Profile::StereoHigh => (H264Profile::IDC_STEREO_HIGH, 0),
        };
        format!("{profile_idc:02x}{profile_iop:02x}")
    }

    /// Try to convert the `str` to a `H264Profile`.
    pub fn from_str(src: &str) -> Result<H264Profile, ()> {
        let bytes = src.as_bytes();
        let idc_str = std::str::from_utf8(&bytes[..2]).map_err(|_| ())?;
        let iop_str = std::str::from_utf8(&bytes[2..4]).map_err(|_| ())?;
        let idc = u8::from_str_radix(idc_str, 16).map_err(|_| ())?;
        let iop = u8::from_str_radix(iop_str, 16).map_err(|_| ())?;

        // Table 5 of RFC6184.
        //
        //   Profile     profile_idc        profile-iop
        //               (hexadecimal)      (binary)

        //   CB          42 (B)             x1xx0000
        //      same as: 4D (M)             1xxx0000
        //      same as: 58 (E)             11xx0000
        //   B           42 (B)             x0xx0000
        //      same as: 58 (E)             10xx0000
        //   M           4D (M)             0x0x0000
        //   E           58                 00xx0000
        //   H           64                 00000000
        //   H10         6E                 00000000
        //   H42         7A                 00000000
        //   H44         F4                 00000000
        //   H10I        6E                 00010000
        //   H42I        7A                 00010000
        //   H44I        F4                 00010000
        //   C44I        2C                 00010000

        const BITS_ON_LAST_HALF_MASK: u8 = 0b00001111;

        // FIXME: This is ugly
        match idc {
            H264Profile::IDC_BASELINE => {
                const CONSTRAINED_BASELINE_MASK: u8 = 0b01000000;

                if iop & BITS_ON_LAST_HALF_MASK != 0 {
                    return Err(());
                }

                if iop & CONSTRAINED_BASELINE_MASK != 0 {
                    return Ok(H264Profile::ConstrainedBaseline);
                }

                return Ok(H264Profile::Baseline);
            }
            H264Profile::IDC_MAIN => {
                const CONSTRAINED_BASELINE_MASK: u8 = 0b10000000;
                const VALID_MAIN_MASK: u8 = 0b01010000;

                if iop & BITS_ON_LAST_HALF_MASK != 0 {
                    return Err(());
                }

                if iop & CONSTRAINED_BASELINE_MASK != 0 {
                    return Ok(H264Profile::ConstrainedBaseline);
                }

                if iop & !VALID_MAIN_MASK != 0 {
                    return Err(());
                }

                return Ok(H264Profile::Main);
            }
            H264Profile::IDC_EXTENDED => {
                const BASELINE_MASK: u8 = 0b10000000;
                const CONSTRAINED_BASELINE_MASK: u8 = 0b11000000;
                const VALID_EXTENDED_MASK: u8 = 0b00110000;

                if iop & BITS_ON_LAST_HALF_MASK != 0 {
                    return Err(());
                }

                if iop & CONSTRAINED_BASELINE_MASK != 0 {
                    return Ok(H264Profile::ConstrainedBaseline);
                }

                if iop & BASELINE_MASK != 0 {
                    return Ok(H264Profile::Baseline);
                }

                if iop & !VALID_EXTENDED_MASK != 0 {
                    return Err(());
                }

                return Ok(H264Profile::Extended);
            }
            H264Profile::IDC_HIGH => match iop {
                0 => return Ok(H264Profile::High),
                0b00001000 => return Ok(H264Profile::ProgressiveHigh),
                0b00001100 => return Ok(H264Profile::ConstrainedHigh),
                _ => (),
            },
            H264Profile::IDC_HIGH_10 => match iop {
                0 => return Ok(H264Profile::High10),
                0b00010000 => return Ok(H264Profile::High10Intra),
                _ => (),
            },
            H264Profile::IDC_HIGH_422 => match iop {
                0 => return Ok(H264Profile::High422),
                0b00010000 => return Ok(H264Profile::High422Intra),
                _ => (),
            },
            H264Profile::IDC_HIGH_444 => match iop {
                0 => return Ok(H264Profile::High444),
                0b00010000 => return Ok(H264Profile::High444Intra),
                _ => (),
            },
            H264Profile::IDC_CAVLC_444 => match iop {
                0b00010000 => return Ok(H264Profile::Cavlc444Intra),
                _ => (),
            },
            _ => (),
        }
        Err(())
    }
}
