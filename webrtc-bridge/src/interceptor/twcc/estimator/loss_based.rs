//! https://datatracker.ietf.org/doc/html/draft-ietf-rmcat-gcc-02#section-6

pub struct LossBasedBandwidthEstimator;

impl LossBasedBandwidthEstimator {
    pub fn new() -> LossBasedBandwidthEstimator {
        LossBasedBandwidthEstimator {}
    }

    pub fn estimate(&mut self, current_bandwidth: f64, received: u32, lost: u32) -> f64 {
        let total = received + lost;
        let fraction_lost = lost as f64 / total as f64;
        if fraction_lost < 0.02 {
            current_bandwidth * 1.05
        } else if fraction_lost > 0.10 {
            current_bandwidth * (1.0 - 0.5 * fraction_lost)
        } else {
            current_bandwidth
        }
    }
}
