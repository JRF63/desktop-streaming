mod estimator;
mod interceptor;
mod sender;
mod sync;
mod time;

use crate::network::data_rate::DataRate;
pub use interceptor::TwccInterceptorBuilder;
use tokio::sync::watch;

pub type TwccBandwidthEstimate = watch::Receiver<DataRate>;

pub type TwccBandwidthSender = watch::Sender<DataRate>;

/// Create a new channel for sending/receiving the bandwidth estimate.
pub(crate) fn twcc_bandwidth_estimate_channel(
    init: DataRate,
) -> (watch::Sender<DataRate>, watch::Receiver<DataRate>) {
    watch::channel(init)
}
