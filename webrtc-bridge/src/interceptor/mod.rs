pub mod twcc;

use crate::network::data_rate::DataRate;
use twcc::{twcc_bandwidth_estimate_channel, TwccBandwidthEstimate, TwccInterceptorBuilder};
use webrtc::{error::Result, interceptor::registry::Registry};

pub fn configure_custom_twcc_sender(
    mut registry: Registry,
    init_bandwidth: DataRate,
) -> Result<(Registry, TwccBandwidthEstimate)> {
    let (tx, rx) = twcc_bandwidth_estimate_channel(init_bandwidth);
    let builder = TwccInterceptorBuilder::new(tx);
    registry.add(Box::new(builder));
    Ok((registry, rx))
}
