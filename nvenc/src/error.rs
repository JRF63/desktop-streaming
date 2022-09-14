#[repr(i32)]
#[derive(thiserror::Error, Debug)]
pub enum NvEncError {
    // NvENC API errors
    #[error("No encode capable devices were detected.")]
    NoEncodeDevice = 1,
    #[error("Devices pass by the client is not supported.")]
    UnsupportedDevice = 2,
    #[error("Encoder device supplied by the client is not valid.")]
    InvalidEncoderdevice = 3,
    #[error("Device passed to the API call is invalid.")]
    InvalidDevice = 4,
    #[error("Device passed to the API call is no longer available and needs to be reinitialized. The clients need to destroy the current encoder session by freeing the allocated input output buffers and destroying the device and create a new encoding session.")]
    DeviceNotExist = 5,
    #[error("One or more of the pointers passed to the API call is invalid.")]
    InvalidPtr = 6,
    #[error("Completion event passed in ::NvEncEncodePicture() call is invalid.")]
    InvalidEvent = 7,
    #[error("One or more of the parameter passed to the API call is invalid.")]
    InvalidParam = 8,
    #[error("An API call was made in wrong sequence/order.")]
    InvalidCall = 9,
    #[error("API call failed because it was unable to allocate enough memory to perform the requested operation.")]
    OutOfMemory = 10,
    #[error("Encoder has not been initialized with ::NvEncInitializeEncoder() or that initialization has failed. The client cannot allocate input or output buffers or do any encoding related operation before successfully initializing the encoder.")]
    EncoderNotInitialized = 11,
    #[error("Unsupported parameter was passed by the client.")]
    UnsupportedParam = 12,
    #[error("::NvEncLockBitstream() failed to lock the output buffer. This happens when the client makes a non blocking lock call to access the output bitstream by passing NV_ENC_LOCK_BITSTREAM::doNotWait flag. This is not a fatal error and client should retry the same operation after few milliseconds.")]
    LockBusy = 13,
    #[error(
        "Size of the user buffer passed by the client is insufficient for the requested operation."
    )]
    NotEnoughBuffer = 14,
    #[error("Invalid struct version was used by the client.")]
    InvalidVersion = 15,
    #[error("::NvEncMapInputResource() API failed to map the client provided input resource.")]
    MapFailed = 16,
    #[error("Encode driver requires more input buffers to produce an output bitstream. If this error is returned from ::NvEncEncodePicture() API, this is not a fatal error. If the client is encoding with B frames then, ::NvEncEncodePicture() API might be buffering the input frame for re-ordering.  A client operating in synchronous mode cannot call ::NvEncLockBitstream() API on the output bitstream buffer if ::NvEncEncodePicture() returned the ::NV_ENC_ERR_NEED_MORE_INPUT error code. The client must continue providing input frames until encode driver returns ::NV_ENC_SUCCESS. After receiving ::NV_ENC_SUCCESS status the client can call ::NvEncLockBitstream() API on the output buffers in the same order in which it has called ::NvEncEncodePicture().")]
    NeedMoreInput = 17,
    #[error("HW encoder is busy encoding and is unable to encode the input. The client should call ::NvEncEncodePicture() again after few milliseconds.")]
    EncoderBusy = 18,
    #[error("Completion event passed in ::NvEncEncodePicture() API has not been registered with encoder driver using ::NvEncRegisterAsyncEvent().")]
    EventNotRegisterd = 19,
    #[error("An unknown internal error has occurred.")]
    Generic = 20,
    #[error("Client is attempting to use a feature that is not available for the license type for the current system.")]
    IncompatibleClientKey = 21,
    #[error("the client is attempting to use a feature that is not implemented for the current version.")]
    Unimplemented = 22,
    #[error("::NvEncRegisterResource API failed to register the resource.")]
    ResourceRegisterFailed = 23,
    #[error(
        "Client is attempting to unregister a resource that has not been successfully registered."
    )]
    ResourceNotRegistered = 24,
    #[error("Client is attempting to unmap a resource that has not been successfully mapped.")]
    ResourceNotMapped = 25,

    // This library's errors
    #[error("The shared library for `nvEncodeAPI64` is not signed and may have been tampered.")]
    LibraryNotSigned = LIBRARY_ERRORS_OFFSET + 0,
    #[error("Loading the shared library for `nvEncodeAPI64` failed.")]
    LibraryLoadingFailed = LIBRARY_ERRORS_OFFSET + 1,
    #[error("Unable to locate `NvEncodeAPIGetMaxSupportedVersion` in the shared library.")]
    GetMaxSupportedVersionLoadingFailed = LIBRARY_ERRORS_OFFSET + 2,
    #[error("Unable to locate `NvEncodeAPICreateInstance` in the shared library.")]
    CreateInstanceLoadingFailed = LIBRARY_ERRORS_OFFSET + 3,
    #[error("The installed driver does not support the version of the NvEnc API that this library is compiled with.")]
    UnsupportedVersion = LIBRARY_ERRORS_OFFSET + 4,
    #[error("`NvEncodeAPICreateInstance` returned a malformed function list.")]
    MalformedFunctionList = LIBRARY_ERRORS_OFFSET + 5,

    #[error("Could not create a Windows event object")]
    EventObjectCreationFailed = EVENT_OBJECT_ERRORS_OFFSET + 0,
    #[error("Error while waiting for the event object to be signaled")]
    EventObjectWaitError = EVENT_OBJECT_ERRORS_OFFSET + 1,
    #[error("Event timed-out while waiting")]
    EventObjectWaitTimeout = EVENT_OBJECT_ERRORS_OFFSET + 2,
}

const LIBRARY_ERRORS_OFFSET: i32 = 100;
const EVENT_OBJECT_ERRORS_OFFSET: i32 = 200;

impl NvEncError {
    pub fn from_nvenc_status(status: crate::sys::NVENCSTATUS) -> Option<Self> {
        match status {
            crate::sys::NVENCSTATUS::NV_ENC_SUCCESS => None,
            status => {
                let err: NvEncError = unsafe { std::mem::transmute(status) };
                Some(err)
            }
        }
    }
}
