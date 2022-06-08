#![allow(dead_code)]
// use ndk_sys::{
//     AAssetManager, AAssetManager_open, AAsset_close, AAsset_getRemainingLength64, AAsset_read,
//     AASSET_MODE_STREAMING,
// };
// use std::ffi::CString;

// fn read_asset(asset_manager: *mut AAssetManager, filename: &str) -> anyhow::Result<Vec<u8>> {
//     let filename = CString::new(filename).unwrap();
//     let asset =
//         unsafe { AAssetManager_open(asset_manager, filename.as_ptr(), AASSET_MODE_STREAMING as _) };
//     if asset.is_null() {
//         anyhow::bail!("{} not found", filename.into_string().unwrap());
//     } else {
//         let buf_size = unsafe { AAsset_getRemainingLength64(asset) };
//         let mut buf = vec![0; buf_size as usize];
//         unsafe {
//             let bytes = AAsset_read(asset, buf.as_mut_ptr().cast(), buf_size as u64);
//             AAsset_close(asset);
//             if bytes as i64 != buf_size {
//                 anyhow::bail!("Failed reading {}", filename.into_string().unwrap());
//             }
//         }
//         Ok(buf)
//     }
// }

// pub fn get_h264_packets(asset_manager: *mut AAssetManager) -> anyhow::Result<Vec<Vec<u8>>> {
//     let mut packets = Vec::new();
//     for i in 0..120 {
//         let buf = read_asset(asset_manager, &format!("{}.h264", i))?;
//         packets.push(buf);
//     }
//     Ok(packets)
// }

// pub fn get_csd(asset_manager: *mut AAssetManager) -> anyhow::Result<Vec<u8>> {
//     read_asset(asset_manager, "csd.bin")
// }

pub const PACKETS: [&'static [u8]; 120] = [
    NAL_0.as_slice(),
    NAL_1.as_slice(),
    NAL_2.as_slice(),
    NAL_3.as_slice(),
    NAL_4.as_slice(),
    NAL_5.as_slice(),
    NAL_6.as_slice(),
    NAL_7.as_slice(),
    NAL_8.as_slice(),
    NAL_9.as_slice(),
    NAL_10.as_slice(),
    NAL_11.as_slice(),
    NAL_12.as_slice(),
    NAL_13.as_slice(),
    NAL_14.as_slice(),
    NAL_15.as_slice(),
    NAL_16.as_slice(),
    NAL_17.as_slice(),
    NAL_18.as_slice(),
    NAL_19.as_slice(),
    NAL_20.as_slice(),
    NAL_21.as_slice(),
    NAL_22.as_slice(),
    NAL_23.as_slice(),
    NAL_24.as_slice(),
    NAL_25.as_slice(),
    NAL_26.as_slice(),
    NAL_27.as_slice(),
    NAL_28.as_slice(),
    NAL_29.as_slice(),
    NAL_30.as_slice(),
    NAL_31.as_slice(),
    NAL_32.as_slice(),
    NAL_33.as_slice(),
    NAL_34.as_slice(),
    NAL_35.as_slice(),
    NAL_36.as_slice(),
    NAL_37.as_slice(),
    NAL_38.as_slice(),
    NAL_39.as_slice(),
    NAL_40.as_slice(),
    NAL_41.as_slice(),
    NAL_42.as_slice(),
    NAL_43.as_slice(),
    NAL_44.as_slice(),
    NAL_45.as_slice(),
    NAL_46.as_slice(),
    NAL_47.as_slice(),
    NAL_48.as_slice(),
    NAL_49.as_slice(),
    NAL_50.as_slice(),
    NAL_51.as_slice(),
    NAL_52.as_slice(),
    NAL_53.as_slice(),
    NAL_54.as_slice(),
    NAL_55.as_slice(),
    NAL_56.as_slice(),
    NAL_57.as_slice(),
    NAL_58.as_slice(),
    NAL_59.as_slice(),
    NAL_60.as_slice(),
    NAL_61.as_slice(),
    NAL_62.as_slice(),
    NAL_63.as_slice(),
    NAL_64.as_slice(),
    NAL_65.as_slice(),
    NAL_66.as_slice(),
    NAL_67.as_slice(),
    NAL_68.as_slice(),
    NAL_69.as_slice(),
    NAL_70.as_slice(),
    NAL_71.as_slice(),
    NAL_72.as_slice(),
    NAL_73.as_slice(),
    NAL_74.as_slice(),
    NAL_75.as_slice(),
    NAL_76.as_slice(),
    NAL_77.as_slice(),
    NAL_78.as_slice(),
    NAL_79.as_slice(),
    NAL_80.as_slice(),
    NAL_81.as_slice(),
    NAL_82.as_slice(),
    NAL_83.as_slice(),
    NAL_84.as_slice(),
    NAL_85.as_slice(),
    NAL_86.as_slice(),
    NAL_87.as_slice(),
    NAL_88.as_slice(),
    NAL_89.as_slice(),
    NAL_90.as_slice(),
    NAL_91.as_slice(),
    NAL_92.as_slice(),
    NAL_93.as_slice(),
    NAL_94.as_slice(),
    NAL_95.as_slice(),
    NAL_96.as_slice(),
    NAL_97.as_slice(),
    NAL_98.as_slice(),
    NAL_99.as_slice(),
    NAL_100.as_slice(),
    NAL_101.as_slice(),
    NAL_102.as_slice(),
    NAL_103.as_slice(),
    NAL_104.as_slice(),
    NAL_105.as_slice(),
    NAL_106.as_slice(),
    NAL_107.as_slice(),
    NAL_108.as_slice(),
    NAL_109.as_slice(),
    NAL_110.as_slice(),
    NAL_111.as_slice(),
    NAL_112.as_slice(),
    NAL_113.as_slice(),
    NAL_114.as_slice(),
    NAL_115.as_slice(),
    NAL_116.as_slice(),
    NAL_117.as_slice(),
    NAL_118.as_slice(),
    NAL_119.as_slice(),
];

pub const CSD: &'static [u8; 34] = include_bytes!("nals/csd.h264");
pub const NAL_0: &'static [u8; 182094] = include_bytes!("nals/0.h264");
pub const NAL_1: &'static [u8; 40666] = include_bytes!("nals/1.h264");
pub const NAL_2: &'static [u8; 24521] = include_bytes!("nals/2.h264");
pub const NAL_3: &'static [u8; 12573] = include_bytes!("nals/3.h264");
pub const NAL_4: &'static [u8; 2369] = include_bytes!("nals/4.h264");
pub const NAL_5: &'static [u8; 5841] = include_bytes!("nals/5.h264");
pub const NAL_6: &'static [u8; 6496] = include_bytes!("nals/6.h264");
pub const NAL_7: &'static [u8; 14091] = include_bytes!("nals/7.h264");
pub const NAL_8: &'static [u8; 22823] = include_bytes!("nals/8.h264");
pub const NAL_9: &'static [u8; 30327] = include_bytes!("nals/9.h264");
pub const NAL_10: &'static [u8; 11841] = include_bytes!("nals/10.h264");
pub const NAL_11: &'static [u8; 32043] = include_bytes!("nals/11.h264");
pub const NAL_12: &'static [u8; 45825] = include_bytes!("nals/12.h264");
pub const NAL_13: &'static [u8; 79612] = include_bytes!("nals/13.h264");
pub const NAL_14: &'static [u8; 60688] = include_bytes!("nals/14.h264");
pub const NAL_15: &'static [u8; 53922] = include_bytes!("nals/15.h264");
pub const NAL_16: &'static [u8; 39157] = include_bytes!("nals/16.h264");
pub const NAL_17: &'static [u8; 66384] = include_bytes!("nals/17.h264");
pub const NAL_18: &'static [u8; 77466] = include_bytes!("nals/18.h264");
pub const NAL_19: &'static [u8; 123821] = include_bytes!("nals/19.h264");
pub const NAL_20: &'static [u8; 55896] = include_bytes!("nals/20.h264");
pub const NAL_21: &'static [u8; 29886] = include_bytes!("nals/21.h264");
pub const NAL_22: &'static [u8; 137815] = include_bytes!("nals/22.h264");
pub const NAL_23: &'static [u8; 54430] = include_bytes!("nals/23.h264");
pub const NAL_24: &'static [u8; 31753] = include_bytes!("nals/24.h264");
pub const NAL_25: &'static [u8; 43243] = include_bytes!("nals/25.h264");
pub const NAL_26: &'static [u8; 73195] = include_bytes!("nals/26.h264");
pub const NAL_27: &'static [u8; 43752] = include_bytes!("nals/27.h264");
pub const NAL_28: &'static [u8; 73096] = include_bytes!("nals/28.h264");
pub const NAL_29: &'static [u8; 47180] = include_bytes!("nals/29.h264");
pub const NAL_30: &'static [u8; 29961] = include_bytes!("nals/30.h264");
pub const NAL_31: &'static [u8; 28012] = include_bytes!("nals/31.h264");
pub const NAL_32: &'static [u8; 26731] = include_bytes!("nals/32.h264");
pub const NAL_33: &'static [u8; 26307] = include_bytes!("nals/33.h264");
pub const NAL_34: &'static [u8; 25630] = include_bytes!("nals/34.h264");
pub const NAL_35: &'static [u8; 25742] = include_bytes!("nals/35.h264");
pub const NAL_36: &'static [u8; 25754] = include_bytes!("nals/36.h264");
pub const NAL_37: &'static [u8; 27924] = include_bytes!("nals/37.h264");
pub const NAL_38: &'static [u8; 25221] = include_bytes!("nals/38.h264");
pub const NAL_39: &'static [u8; 25566] = include_bytes!("nals/39.h264");
pub const NAL_40: &'static [u8; 25660] = include_bytes!("nals/40.h264");
pub const NAL_41: &'static [u8; 25851] = include_bytes!("nals/41.h264");
pub const NAL_42: &'static [u8; 24086] = include_bytes!("nals/42.h264");
pub const NAL_43: &'static [u8; 24488] = include_bytes!("nals/43.h264");
pub const NAL_44: &'static [u8; 41750] = include_bytes!("nals/44.h264");
pub const NAL_45: &'static [u8; 15775] = include_bytes!("nals/45.h264");
pub const NAL_46: &'static [u8; 41725] = include_bytes!("nals/46.h264");
pub const NAL_47: &'static [u8; 15471] = include_bytes!("nals/47.h264");
pub const NAL_48: &'static [u8; 41524] = include_bytes!("nals/48.h264");
pub const NAL_49: &'static [u8; 41219] = include_bytes!("nals/49.h264");
pub const NAL_50: &'static [u8; 40921] = include_bytes!("nals/50.h264");
pub const NAL_51: &'static [u8; 15524] = include_bytes!("nals/51.h264");
pub const NAL_52: &'static [u8; 38357] = include_bytes!("nals/52.h264");
pub const NAL_53: &'static [u8; 15376] = include_bytes!("nals/53.h264");
pub const NAL_54: &'static [u8; 37409] = include_bytes!("nals/54.h264");
pub const NAL_55: &'static [u8; 37588] = include_bytes!("nals/55.h264");
pub const NAL_56: &'static [u8; 37707] = include_bytes!("nals/56.h264");
pub const NAL_57: &'static [u8; 36948] = include_bytes!("nals/57.h264");
pub const NAL_58: &'static [u8; 37450] = include_bytes!("nals/58.h264");
pub const NAL_59: &'static [u8; 37772] = include_bytes!("nals/59.h264");
pub const NAL_60: &'static [u8; 36692] = include_bytes!("nals/60.h264");
pub const NAL_61: &'static [u8; 36434] = include_bytes!("nals/61.h264");
pub const NAL_62: &'static [u8; 38152] = include_bytes!("nals/62.h264");
pub const NAL_63: &'static [u8; 15483] = include_bytes!("nals/63.h264");
pub const NAL_64: &'static [u8; 36560] = include_bytes!("nals/64.h264");
pub const NAL_65: &'static [u8; 36671] = include_bytes!("nals/65.h264");
pub const NAL_66: &'static [u8; 36324] = include_bytes!("nals/66.h264");
pub const NAL_67: &'static [u8; 36527] = include_bytes!("nals/67.h264");
pub const NAL_68: &'static [u8; 36421] = include_bytes!("nals/68.h264");
pub const NAL_69: &'static [u8; 36511] = include_bytes!("nals/69.h264");
pub const NAL_70: &'static [u8; 36123] = include_bytes!("nals/70.h264");
pub const NAL_71: &'static [u8; 36667] = include_bytes!("nals/71.h264");
pub const NAL_72: &'static [u8; 36452] = include_bytes!("nals/72.h264");
pub const NAL_73: &'static [u8; 36393] = include_bytes!("nals/73.h264");
pub const NAL_74: &'static [u8; 36584] = include_bytes!("nals/74.h264");
pub const NAL_75: &'static [u8; 12769] = include_bytes!("nals/75.h264");
pub const NAL_76: &'static [u8; 36784] = include_bytes!("nals/76.h264");
pub const NAL_77: &'static [u8; 36491] = include_bytes!("nals/77.h264");
pub const NAL_78: &'static [u8; 36750] = include_bytes!("nals/78.h264");
pub const NAL_79: &'static [u8; 36736] = include_bytes!("nals/79.h264");
pub const NAL_80: &'static [u8; 35780] = include_bytes!("nals/80.h264");
pub const NAL_81: &'static [u8; 36078] = include_bytes!("nals/81.h264");
pub const NAL_82: &'static [u8; 34586] = include_bytes!("nals/82.h264");
pub const NAL_83: &'static [u8; 15245] = include_bytes!("nals/83.h264");
pub const NAL_84: &'static [u8; 35254] = include_bytes!("nals/84.h264");
pub const NAL_85: &'static [u8; 35681] = include_bytes!("nals/85.h264");
pub const NAL_86: &'static [u8; 35054] = include_bytes!("nals/86.h264");
pub const NAL_87: &'static [u8; 35725] = include_bytes!("nals/87.h264");
pub const NAL_88: &'static [u8; 34184] = include_bytes!("nals/88.h264");
pub const NAL_89: &'static [u8; 15197] = include_bytes!("nals/89.h264");
pub const NAL_90: &'static [u8; 35198] = include_bytes!("nals/90.h264");
pub const NAL_91: &'static [u8; 34880] = include_bytes!("nals/91.h264");
pub const NAL_92: &'static [u8; 35027] = include_bytes!("nals/92.h264");
pub const NAL_93: &'static [u8; 35588] = include_bytes!("nals/93.h264");
pub const NAL_94: &'static [u8; 34445] = include_bytes!("nals/94.h264");
pub const NAL_95: &'static [u8; 15063] = include_bytes!("nals/95.h264");
pub const NAL_96: &'static [u8; 34813] = include_bytes!("nals/96.h264");
pub const NAL_97: &'static [u8; 35034] = include_bytes!("nals/97.h264");
pub const NAL_98: &'static [u8; 35251] = include_bytes!("nals/98.h264");
pub const NAL_99: &'static [u8; 35895] = include_bytes!("nals/99.h264");
pub const NAL_100: &'static [u8; 36171] = include_bytes!("nals/100.h264");
pub const NAL_101: &'static [u8; 35789] = include_bytes!("nals/101.h264");
pub const NAL_102: &'static [u8; 35891] = include_bytes!("nals/102.h264");
pub const NAL_103: &'static [u8; 37554] = include_bytes!("nals/103.h264");
pub const NAL_104: &'static [u8; 15258] = include_bytes!("nals/104.h264");
pub const NAL_105: &'static [u8; 38462] = include_bytes!("nals/105.h264");
pub const NAL_106: &'static [u8; 37714] = include_bytes!("nals/106.h264");
pub const NAL_107: &'static [u8; 15601] = include_bytes!("nals/107.h264");
pub const NAL_108: &'static [u8; 11868] = include_bytes!("nals/108.h264");
pub const NAL_109: &'static [u8; 39890] = include_bytes!("nals/109.h264");
pub const NAL_110: &'static [u8; 38869] = include_bytes!("nals/110.h264");
pub const NAL_111: &'static [u8; 39008] = include_bytes!("nals/111.h264");
pub const NAL_112: &'static [u8; 38853] = include_bytes!("nals/112.h264");
pub const NAL_113: &'static [u8; 15640] = include_bytes!("nals/113.h264");
pub const NAL_114: &'static [u8; 11859] = include_bytes!("nals/114.h264");
pub const NAL_115: &'static [u8; 38534] = include_bytes!("nals/115.h264");
pub const NAL_116: &'static [u8; 39226] = include_bytes!("nals/116.h264");
pub const NAL_117: &'static [u8; 40288] = include_bytes!("nals/117.h264");
pub const NAL_118: &'static [u8; 39376] = include_bytes!("nals/118.h264");
pub const NAL_119: &'static [u8; 40654] = include_bytes!("nals/119.h264");
