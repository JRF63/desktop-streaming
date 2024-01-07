mod history;
mod overuse_detector;
mod packet_group;

use self::{
    history::History,
    overuse_detector::{DelayDetector, NetworkCondition},
    packet_group::PacketGroup,
};
use super::TwccTime;
use std::{collections::VecDeque, time::Instant};

const BURST_TIME_US: i64 = 5000;

// Should be within 500 - 1000 ms if packets are grouped by 5 ms burst time
const WINDOW_SIZE: u32 = 100;

const ESTIMATOR_REACTION_TIME_MS: f64 = 100.0;

const STATE_NOISE_COVARIANCE: f64 = 10e-3;

const INITIAL_SYSTEM_ERROR_COVARIANCE: f64 = 0.1;

// Midway between the recommended value of 0.001 - 0.1
const CHI: f64 = 0.01;

const INITIAL_DELAY_THRESHOLD_US: f64 = 12500.0;

const OVERUSE_TIME_THRESHOLD_US: i64 = 10000;

const K_U: f64 = 0.01;

const K_D: f64 = 0.00018;

const DECREASE_RATE_FACTOR: f64 = 0.85;

// Exponential moving average smoothing factor
const ALPHA: f64 = 0.95;

struct IncomingBitrateEstimate {
    mean: f64,
    variance: f64,
    converged: bool,
}

impl IncomingBitrateEstimate {
    fn new() -> IncomingBitrateEstimate {
        IncomingBitrateEstimate {
            mean: 0.0,
            variance: 0.0,
            converged: false,
        }
    }

    fn update(&mut self, bytes_per_sec: f64) {
        let diff = bytes_per_sec - self.mean;
        // Check if sample is beyond 3 stddevs away from the mean
        if diff * diff > 9.0 * self.variance {
            // Reset the average and go to multiplicative increase
            self.mean = bytes_per_sec;
            self.variance = 0.0;
            self.converged = false;
            return;
        } else {
            self.converged = true;
        }

        // Exponentially-weighted mean and variance calculation from:
        // https://web.archive.org/web/20181222175223/http://people.ds.cam.ac.uk/fanf2/hermes/doc/antiforgery/stats.pdf
        let incr = ALPHA * diff;
        self.mean = self.mean + incr;
        self.variance = (1.0 - ALPHA) * (self.variance + diff * incr);
    }

    fn has_converged(&self) -> bool {
        self.converged
    }
}

pub struct DelayBasedBandwidthEstimator {
    prev_group: Option<PacketGroup>,
    curr_group: Option<PacketGroup>,
    history: History,
    incoming_bitrate_estimate: IncomingBitrateEstimate,
    delay_detector: Option<DelayDetector>,
    last_update: Option<Instant>,
    network_condition: NetworkCondition,
    rtt_ms: f64,
}

impl DelayBasedBandwidthEstimator {
    pub fn new() -> DelayBasedBandwidthEstimator {
        DelayBasedBandwidthEstimator {
            prev_group: None,
            curr_group: None,
            history: History::new(),
            incoming_bitrate_estimate: IncomingBitrateEstimate::new(),
            delay_detector: None,
            last_update: None,
            network_condition: NetworkCondition::Normal,
            rtt_ms: 0.0,
        }
    }

    pub fn process_packet(
        &mut self,
        departure_time: TwccTime,
        arrival_time: TwccTime,
        packet_size: u64,
    ) {
        let mut new_packet_group = false;

        if let Some(curr_group) = &mut self.curr_group {
            // Ignore reordered packets
            if departure_time >= curr_group.earliest_departure_time_us {
                if curr_group.belongs_to_group(departure_time, arrival_time) {
                    curr_group.add_packet(departure_time, arrival_time, packet_size);
                } else {
                    new_packet_group = true;
                }
            }
        } else {
            new_packet_group = true;
        }

        if new_packet_group {
            self.curr_group_completed(arrival_time);

            std::mem::swap(&mut self.prev_group, &mut self.curr_group);
            self.curr_group = Some(PacketGroup::new(departure_time, arrival_time, packet_size));
        }
    }

    pub fn update_rtt(&mut self, rtt_ms: f64) {
        self.rtt_ms = rtt_ms;
    }

    fn curr_group_completed(&mut self, arrival_time: TwccTime) {
        if let (Some(curr_group), Some(prev_group)) = (&self.curr_group, &self.prev_group) {
            // Inter-departure time should be >= 0 since we ignore reordered packets
            let interdeparture_time = curr_group.interdeparture_time(prev_group);
            let interarrival_time = curr_group.interarrival_time(prev_group);
            let intergroup_delay = interarrival_time - interdeparture_time;

            self.history.add_group(curr_group, interdeparture_time);

            if let Some(delay_detector) = &mut self.delay_detector {
                if let Some(&min_send_interval) = self.history.smallest_send_interval() {
                    self.network_condition = delay_detector.detect_network_condition(
                        intergroup_delay,
                        min_send_interval,
                        interarrival_time,
                        arrival_time,
                    );
                }
            } else {
                self.delay_detector = Some(DelayDetector::new(intergroup_delay));
            }
        }
    }

    pub fn estimate(&mut self, current_bandwidth: f64, now: Instant) -> f64 {
        // Underuse - retain current bandwidth (bugged?)
        // Normal - increase bandwidth
        // Overuse - decrease bandwidth
        let mut bandwidth_estimate = match self.network_condition {
            NetworkCondition::Underuse | NetworkCondition::Normal => {
                let time_since_last_update_ms = self.time_since_last_update(now);

                if self.incoming_bitrate_estimate.has_converged() {
                    bandwidth_additive_increase(
                        current_bandwidth,
                        time_since_last_update_ms,
                        self.rtt_ms,
                        self.history.average_packet_size_bytes(),
                    )
                } else {
                    bandwidth_multiplicative_increase(current_bandwidth, time_since_last_update_ms)
                }
            }
            NetworkCondition::Overuse => {
                if let Some(received_bandwidth) = self.history.received_bandwidth_bytes_per_sec() {
                    self.incoming_bitrate_estimate.update(received_bandwidth);
                    bandwidth_decrease(received_bandwidth)
                } else {
                    // We don't have an estimate of the received bandwidth but we still want to
                    // decrease the sending bandwidth. Use the current sending bandwidth as a proxy
                    // assuming it's near the received bandwidth.
                    bandwidth_decrease(current_bandwidth)
                }
            }
        };
        self.last_update = Some(now);

        // Cap the bandwidth to a multiple of the apparent bandwidth on the receiver size
        if let Some(received_bandwidth) = self.history.received_bandwidth_bytes_per_sec() {
            let bandwidth_threshold = 1.5 * received_bandwidth;
            if bandwidth_estimate >= bandwidth_threshold {
                bandwidth_estimate = bandwidth_threshold;
            }
        }

        return bandwidth_estimate;
    }

    fn time_since_last_update(&self, now: Instant) -> f64 {
        let millis = self
            .last_update
            .map(|t| now.duration_since(t).as_millis() as f64)
            .unwrap_or((BURST_TIME_US / 1000) as f64);
        millis
    }
}

fn bandwidth_additive_increase(
    current_bandwidth: f64,
    time_since_last_update_ms: f64,
    rtt_ms: f64,
    ave_packet_size_bytes: f64,
) -> f64 {
    let response_time_ms = ESTIMATOR_REACTION_TIME_MS + rtt_ms;

    let alpha = 0.5 * f64::min(1.0, time_since_last_update_ms / response_time_ms);
    // Bandwidth is in bytes/s hence the 1000 in the congestion control draft was divided by 8
    current_bandwidth + f64::max(125.0, alpha * ave_packet_size_bytes)
}

fn bandwidth_multiplicative_increase(
    current_bandwidth: f64,
    time_since_last_update_ms: f64,
) -> f64 {
    let eta = 1.08f64.powf(f64::min(1.0, time_since_last_update_ms / 1000.0));
    current_bandwidth * eta
}

fn bandwidth_decrease(received_bandwidth: f64) -> f64 {
    received_bandwidth * DECREASE_RATE_FACTOR
}
