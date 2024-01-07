use webrtc::rtp::header::Header;

pub trait RtpHeaderExt {
    fn advance_sequence_number(&mut self);
}

impl RtpHeaderExt for Header {
    #[inline]
    fn advance_sequence_number(&mut self) {
        self.sequence_number = self.sequence_number.wrapping_add(1);
    }
}
