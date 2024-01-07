use crate::codecs::h264::constants::SPS_NALU_TYPE;
use exp_golomb::ExpGolombDecoder;

const NALU_TYPE_BITMASK: u8 = 0x1F;

pub fn parse_parameter_sets_for_resolution(buf: &[u8]) -> Option<(usize, usize)> {
    // Start past the NAL delimiter
    let mut offset = 'outer: {
        let mut zeroes = 0;
        for (i, &byte) in buf.iter().enumerate() {
            match byte {
                0 => zeroes += 1,
                1 => {
                    if zeroes >= 2 {
                        let candidate = i + 1;
                        // Data is found in the SPS
                        if buf.get(candidate)? & NALU_TYPE_BITMASK == SPS_NALU_TYPE {
                            break 'outer candidate;
                        }
                    }
                    zeroes = 0;
                }
                _ => zeroes = 0,
            }
        }

        // Reached end of buffer, no NAL delimiter
        0
    };

    // Skip nal_unit_type
    offset += 1;

    let profile_idc = buf[offset];

    // Skip constraint_sets, level_idc
    offset += 3;

    let mut exp_golomb = ExpGolombDecoder::new(&buf[offset..], 0)?;

    // Skip seq_parameter_set_id
    exp_golomb.skip_next();

    if let 100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 13 = profile_idc {
        let chroma_format_idc = exp_golomb.next_unsigned()?;

        if chroma_format_idc == 3 {
            // Skip separate_colour_plane_flag
            exp_golomb.next_bit()?;
        }

        // Skip bit_depth_luma_minus8
        exp_golomb.skip_next();
        // Skip bit_depth_chroma_minus8
        exp_golomb.skip_next();

        // Skip qpprime_y_zero_transform_bypass_flag
        exp_golomb.next_bit()?;

        let seq_scaling_matrix_present_flag = exp_golomb.next_bit()?;
        if seq_scaling_matrix_present_flag == 1 {
            let _count = if chroma_format_idc != 3 { 8 } else { 12 };
            // scaling_list not implemented
            todo!();
        }
    }

    // Skip log2_max_frame_num_minus4
    exp_golomb.skip_next();

    let pic_order_cnt_type = exp_golomb.next_unsigned()?;
    if pic_order_cnt_type == 0 {
        // Skip log2_max_pic_order_cnt_lsb_minus4
        exp_golomb.skip_next();
    } else if pic_order_cnt_type == 1 {
        // Skip delta_pic_order_always_zero_flag
        exp_golomb.next_bit()?;
        // Skip offset_for_non_ref_pic
        exp_golomb.skip_next();
        // Skip offset_for_top_to_bottom_field
        exp_golomb.skip_next();

        let num_ref_frames_in_pic_order_cnt_cycle = exp_golomb.next_unsigned()?;
        for _ in 0..num_ref_frames_in_pic_order_cnt_cycle {
            // Skip offset_for_ref_frame
            exp_golomb.skip_next();
        }
    }

    // Skip max_num_ref_frames
    exp_golomb.skip_next();
    // Skip gaps_in_frame_num_value_allowed_flag
    exp_golomb.next_bit()?;

    let pic_width_in_mbs_minus1 = exp_golomb.next_unsigned()?;
    let pic_height_in_map_units_minus1 = exp_golomb.next_unsigned()?;
    let frame_mbs_only_flag = exp_golomb.next_bit()?;

    if frame_mbs_only_flag == 0 {
        // Skip mb_adaptive_frame_field_flag
        exp_golomb.next_bit()?;
    }

    // Skip direct_8x8_inference_flag
    exp_golomb.next_bit()?;
    let frame_cropping_flag = exp_golomb.next_bit()?;

    // These are interpreted as 0 if frame_cropping_flag == 0
    let mut frame_crop_left_offset = 0;
    let mut frame_crop_right_offset = 0;
    let mut frame_crop_top_offset = 0;
    let mut frame_crop_bottom_offset = 0;
    if frame_cropping_flag == 1 {
        frame_crop_left_offset = exp_golomb.next_unsigned()?;
        frame_crop_right_offset = exp_golomb.next_unsigned()?;
        frame_crop_top_offset = exp_golomb.next_unsigned()?;
        frame_crop_bottom_offset = exp_golomb.next_unsigned()?;
    }

    let width = 16 * (pic_width_in_mbs_minus1 + 1)
        - frame_crop_right_offset * 2
        - frame_crop_left_offset * 2;

    let height = 16 * (2 - frame_mbs_only_flag as u64) * (pic_height_in_map_units_minus1 + 1)
        - frame_crop_top_offset * 2
        - frame_crop_bottom_offset * 2;

    return Some((width as usize, height as usize));
}

#[test]
fn sps_parse() {
    const NALU: &'static [u8] = include_bytes!("nalus/csd.bin");
    assert_eq!(
        parse_parameter_sets_for_resolution(NALU),
        Some((1920, 1080))
    );
}
