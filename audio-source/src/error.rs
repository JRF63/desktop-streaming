/// Errors that can be returned by an audio source.
#[derive(thiserror::Error, Debug)]
pub enum AudioSourceError {
    #[error(
        "Audio device was removed or disabled. \
        The caller is suggested to create a new audio source to handle this error."
    )]
    DeviceInvalidated,
    #[error(
        "Wait timeout elapsed. This error is recoverable just by retrying the offending function."
    )]
    WaitTimeout,
    #[error("Internal error occured. This error is not recoverable.")]
    InternalError,
}
