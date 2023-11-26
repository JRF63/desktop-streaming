/// Errors that can be returned by a video source.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Internal error occured. This error is not recoverable.")]
    InternalError,
    #[error(
        "Wait timeout elapsed. This error is recoverable just by retrying the offending function."
    )]
    WaitTimeout,
    #[error("The interface became invalidated and needs to be recreated.")]
    AccessLost,
}
