use super::{sync::TwccSendInfo, time::TwccTime};
use async_trait::async_trait;
use std::{sync::Arc, time::Instant};
use webrtc::{
    interceptor::{Attributes, Error, RTPWriter},
    rtp::{self, extension::transport_cc_extension::TransportCcExtension},
    util::Unmarshal,
};

pub struct TwccTimestampSenderStream {
    map: TwccSendInfo,
    hdr_ext_id: u8,
    next_writer: Arc<dyn RTPWriter + Send + Sync>,
    start_time: Instant,
}

impl TwccTimestampSenderStream {
    pub fn new(
        map: TwccSendInfo,
        hdr_ext_id: u8,
        next_writer: Arc<dyn RTPWriter + Send + Sync>,
        start_time: Instant,
    ) -> TwccTimestampSenderStream {
        TwccTimestampSenderStream {
            map,
            hdr_ext_id,
            next_writer,
            start_time,
        }
    }
}

#[async_trait]
impl RTPWriter for TwccTimestampSenderStream {
    async fn write(
        &self,
        pkt: &rtp::packet::Packet,
        attributes: &Attributes,
    ) -> Result<usize, Error> {
        // `TwccExtensionCapturerStream` must run after `TransportCcExtension` has been set
        if let Some(mut buf) = pkt.header.get_extension(self.hdr_ext_id) {
            // Incoming bitrate measured, R_hat, only considers payload size:
            // https://datatracker.ietf.org/doc/html/draft-ietf-rmcat-gcc-02#section-5.5
            let payload_size = pkt.payload.len() as u64;

            let tcc_ext = TransportCcExtension::unmarshal(&mut buf)?;
            let timestamp = Instant::now().duration_since(self.start_time);
            self.map.store_send_info(
                tcc_ext.transport_sequence,
                TwccTime::from_duration(&timestamp),
                payload_size,
            );
        }
        self.next_writer.write(pkt, attributes).await
    }
}
