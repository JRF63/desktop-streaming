mod delay_based;
mod loss_based;

use webrtc::rtcp::transport_feedbacks::transport_layer_cc::{
    PacketStatusChunk, SymbolTypeTcc, TransportLayerCc,
};

use self::{delay_based::DelayBasedBandwidthEstimator, loss_based::LossBasedBandwidthEstimator};
use super::{sync::TwccSendInfo, time::TwccTime, TwccBandwidthSender};
use crate::network::data_rate::DataRate;
use std::time::Instant;

pub struct TwccBandwidthEstimator {
    estimate_sender: TwccBandwidthSender,
    delay_based_estimator: DelayBasedBandwidthEstimator,
    loss_based_estimator: LossBasedBandwidthEstimator,
    received: u32,
    lost: u32,
}

impl TwccBandwidthEstimator {
    pub fn new(estimate_sender: TwccBandwidthSender) -> TwccBandwidthEstimator {
        TwccBandwidthEstimator {
            estimate_sender,
            delay_based_estimator: DelayBasedBandwidthEstimator::new(),
            loss_based_estimator: LossBasedBandwidthEstimator::new(),
            received: 0,
            lost: 0,
        }
    }

    pub fn estimate(&mut self, now: Instant) {
        let current_bandwidth = self.estimate_sender.borrow().bytes_per_sec_f64();
        let a = self.delay_based_estimator.estimate(current_bandwidth, now);
        let b = self
            .loss_based_estimator
            .estimate(current_bandwidth, self.received, self.lost);
        let bandwidth = f64::min(a, b);
        self.estimate_sender
            .send_if_modified(|data_rate: &mut DataRate| {
                if bandwidth == current_bandwidth {
                    false
                } else {
                    *data_rate = DataRate::from_bytes_per_sec_f64(bandwidth);
                    true
                }
            });

        self.received = 0;
        self.lost = 0;
    }

    pub fn process_feedback(&mut self, tcc: &TransportLayerCc, send_info: &TwccSendInfo) {
        let mut sequence_number = tcc.base_sequence_number;
        let mut arrival_time = TwccTime::extract_from_rtcp(tcc);

        let mut recv_deltas_iter = tcc.recv_deltas.iter();

        let mut with_packet_status = |status: &SymbolTypeTcc| {
            match status {
                SymbolTypeTcc::PacketNotReceived => {
                    self.lost += 1;
                }
                SymbolTypeTcc::PacketReceivedWithoutDelta => {
                    self.received += 1;
                }
                _ => {
                    self.received += 1;
                    if let Some(recv_delta) = recv_deltas_iter.next() {
                        arrival_time = TwccTime::from_recv_delta(arrival_time, recv_delta);

                        let (departure_time, packet_size) =
                            send_info.load_send_info(sequence_number);

                        self.delay_based_estimator.process_packet(
                            departure_time,
                            arrival_time,
                            packet_size,
                        );
                    }
                }
            }
            sequence_number = sequence_number.wrapping_add(1);
        };

        for chunk in tcc.packet_chunks.iter() {
            match chunk {
                PacketStatusChunk::RunLengthChunk(chunk) => {
                    for _ in 0..chunk.run_length {
                        with_packet_status(&chunk.packet_status_symbol);
                    }
                }
                PacketStatusChunk::StatusVectorChunk(chunk) => {
                    for status in chunk.symbol_list.iter() {
                        with_packet_status(status);
                    }
                }
            }
        }
    }

    pub fn update_rtt(&mut self, rtt_ms: f64) {
        self.delay_based_estimator.update_rtt(rtt_ms);
    }
}
