use std::num::NonZeroI32;

#[derive(Debug)]
#[repr(transparent)]
pub struct NvEncError(NonZeroI32);

impl NvEncError {
    pub(crate) const fn new(status: nvenc_sys::NVENCSTATUS) -> Option<Self> {
        match NonZeroI32::new(status as i32) {
            Some(status) => Some(NvEncError(status)),
            None => None
        }
    }

    fn as_nvenc_status(&self) -> nvenc_sys::NVENCSTATUS {
        unsafe { std::mem::transmute(self.0) }
    }
}

impl std::error::Error for NvEncError {}

impl std::fmt::Display for NvEncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let err_msg = match self.as_nvenc_status() {
            nvenc_sys::NVENCSTATUS::NV_ENC_SUCCESS => "API call returned with no errors.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_NO_ENCODE_DEVICE => "No encode capable devices were detected.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_UNSUPPORTED_DEVICE => "Devices pass by the client is not supported.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_INVALID_ENCODERDEVICE => "Encoder device supplied by the client is not valid.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_INVALID_DEVICE => "Device passed to the API call is invalid.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_DEVICE_NOT_EXIST => "Device passed to the API call is no longer available and needs to be reinitialized. The clients need to destroy the current encoder session by freeing the allocated input output buffers and destroying the device and create a new encoding session.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_INVALID_PTR => "One or more of the pointers passed to the API call is invalid.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_INVALID_EVENT => "Completion event passed in ::NvEncEncodePicture() call is invalid.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_INVALID_PARAM => "One or more of the parameter passed to the API call is invalid.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_INVALID_CALL => "An API call was made in wrong sequence/order.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_OUT_OF_MEMORY => "API call failed because it was unable to allocate enough memory to perform the requested operation.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_ENCODER_NOT_INITIALIZED => "Encoder has not been initialized with ::NvEncInitializeEncoder() or that initialization has failed. The client cannot allocate input or output buffers or do any encoding related operation before successfully initializing the encoder.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_UNSUPPORTED_PARAM => "Unsupported parameter was passed by the client.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_LOCK_BUSY => "::NvEncLockBitstream() failed to lock the output buffer. This happens when the client makes a non blocking lock call to access the output bitstream by passing NV_ENC_LOCK_BITSTREAM::doNotWait flag. This is not a fatal error and client should retry the same operation after few milliseconds.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_NOT_ENOUGH_BUFFER => "Size of the user buffer passed by the client is insufficient for the requested operation.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_INVALID_VERSION => "Invalid struct version was used by the client.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_MAP_FAILED => "::NvEncMapInputResource() API failed to map the client provided input resource.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_NEED_MORE_INPUT => "Encode driver requires more input buffers to produce an output bitstream. If this error is returned from ::NvEncEncodePicture() API, this is not a fatal error. If the client is encoding with B frames then, ::NvEncEncodePicture() API might be buffering the input frame for re-ordering.  A client operating in synchronous mode cannot call ::NvEncLockBitstream() API on the output bitstream buffer if ::NvEncEncodePicture() returned the ::NV_ENC_ERR_NEED_MORE_INPUT error code. The client must continue providing input frames until encode driver returns ::NV_ENC_SUCCESS. After receiving ::NV_ENC_SUCCESS status the client can call ::NvEncLockBitstream() API on the output buffers in the same order in which it has called ::NvEncEncodePicture().",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_ENCODER_BUSY => "HW encoder is busy encoding and is unable to encode the input. The client should call ::NvEncEncodePicture() again after few milliseconds.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_EVENT_NOT_REGISTERD => "Completion event passed in ::NvEncEncodePicture() API has not been registered with encoder driver using ::NvEncRegisterAsyncEvent().",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_GENERIC => "An unknown internal error has occurred.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_INCOMPATIBLE_CLIENT_KEY => "Client is attempting to use a feature that is not available for the license type for the current system.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_UNIMPLEMENTED => "the client is attempting to use a feature that is not implemented for the current version.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_RESOURCE_REGISTER_FAILED => "::NvEncRegisterResource API failed to register the resource.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_RESOURCE_NOT_REGISTERED => "Client is attempting to unregister a resource that has not been successfully registered.",
            nvenc_sys::NVENCSTATUS::NV_ENC_ERR_RESOURCE_NOT_MAPPED => "Client is attempting to unmap a resource that has not been successfully mapped.",
            _ => panic!("Unknown NVENC error.")
        };

        write!(f, "NvEncError: {}", err_msg)
    }
}
