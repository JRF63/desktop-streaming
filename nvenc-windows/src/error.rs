use crate::ffi;
use std::num::NonZeroI32;

#[repr(transparent)]
pub struct NvEncError(NonZeroI32);

#[repr(i32)]
#[derive(Debug)]
pub enum ExtError {
    LibraryLoading = -1,
    FunctionAddress = -2,
    UnsupportedVersion = -3,
    FunctionList = -4,
    UnsupportedCodec = -5,
}

#[allow(non_upper_case_globals)]
impl NvEncError {
    pub const NoEncodeDevice: NvEncError =
        NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_NO_ENCODE_DEVICE);

    pub const UnsupportedDevice: NvEncError =
        NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_UNSUPPORTED_DEVICE);

    pub const InvalidEncoderdevice: NvEncError =
        NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_INVALID_ENCODERDEVICE);

    pub const InvalidDevice: NvEncError =
        NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_INVALID_DEVICE);

    pub const DeviceNotExist: NvEncError =
        NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_DEVICE_NOT_EXIST);

    pub const InvalidPtr: NvEncError = NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_INVALID_PTR);

    pub const InvalidEvent: NvEncError =
        NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_INVALID_EVENT);

    pub const InvalidParam: NvEncError =
        NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_INVALID_PARAM);

    pub const InvalidCall: NvEncError = NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_INVALID_CALL);

    pub const OutOfMemory: NvEncError = NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_OUT_OF_MEMORY);

    pub const EncoderNotInitialized: NvEncError =
        NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_ENCODER_NOT_INITIALIZED);

    pub const UnsupportedParam: NvEncError =
        NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_UNSUPPORTED_PARAM);

    pub const LockBusy: NvEncError = NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_LOCK_BUSY);

    pub const NotEnoughBuffer: NvEncError =
        NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_NOT_ENOUGH_BUFFER);

    pub const InvalidVersion: NvEncError =
        NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_INVALID_VERSION);

    pub const MapFailed: NvEncError = NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_MAP_FAILED);

    pub const NeedMoreInput: NvEncError =
        NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_NEED_MORE_INPUT);

    pub const EncoderBusy: NvEncError = NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_ENCODER_BUSY);

    pub const EventNotRegisterd: NvEncError =
        NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_EVENT_NOT_REGISTERD);

    pub const Generic: NvEncError = NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_GENERIC);

    pub const IncompatibleClientKey: NvEncError =
        NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_INCOMPATIBLE_CLIENT_KEY);

    pub const Unimplemented: NvEncError =
        NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_UNIMPLEMENTED);

    pub const ResourceRegisterFailed: NvEncError =
        NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_RESOURCE_REGISTER_FAILED);

    pub const ResourceNotRegistered: NvEncError =
        NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_RESOURCE_NOT_REGISTERED);

    pub const ResourceNotMapped: NvEncError =
        NvEncError::new(ffi::NVENCSTATUS::NV_ENC_ERR_RESOURCE_NOT_MAPPED);

    pub const LibraryLoading: NvEncError = NvEncError::from(ExtError::LibraryLoading);

    pub const FunctionAddress: NvEncError = NvEncError::from(ExtError::FunctionAddress);

    pub const UnsupportedVersion: NvEncError = NvEncError::from(ExtError::UnsupportedVersion);

    pub const FunctionList: NvEncError = NvEncError::from(ExtError::FunctionList);

    pub const UnsupportedCodec: NvEncError = NvEncError::from(ExtError::UnsupportedCodec);

    pub(crate) const fn new(status: ffi::NVENCSTATUS) -> Self {
        NvEncError(unsafe { NonZeroI32::new_unchecked(status as i32) })
    }

    pub(crate) const fn from(error: ExtError) -> Self {
        NvEncError(unsafe { NonZeroI32::new_unchecked(error as i32) })
    }
}
