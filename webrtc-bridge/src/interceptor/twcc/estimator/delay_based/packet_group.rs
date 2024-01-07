use super::*;

pub struct PacketGroup {
    pub earliest_departure_time_us: TwccTime,
    pub departure_time_us: TwccTime,
    pub earliest_arrival_time_us: TwccTime,
    pub arrival_time_us: TwccTime,
    pub size_bytes: u64,
    pub num_packets: u64,
}

impl PacketGroup {
    pub fn new(
        departure_time_us: TwccTime,
        arrival_time_us: TwccTime,
        packet_size: u64,
    ) -> PacketGroup {
        PacketGroup {
            earliest_departure_time_us: departure_time_us,
            departure_time_us,
            earliest_arrival_time_us: arrival_time_us,
            arrival_time_us,
            size_bytes: packet_size,
            num_packets: 1,
        }
    }

    pub fn belongs_to_group(&self, departure_time_us: TwccTime, arrival_time_us: TwccTime) -> bool {
        let interdeparture_time =
            departure_time_us.sub_assuming_small_delta(self.earliest_departure_time_us);
        if interdeparture_time < BURST_TIME_US {
            return true;
        }

        let interarrival_time =
            arrival_time_us.sub_assuming_small_delta(self.earliest_arrival_time_us);
        let intergroup_delay = interarrival_time - interdeparture_time;
        if interarrival_time < BURST_TIME_US && intergroup_delay < 0 {
            return true;
        }

        false
    }

    pub fn add_packet(
        &mut self,
        departure_time_us: TwccTime,
        arrival_time_us: TwccTime,
        packet_size: u64,
    ) {
        self.size_bytes += packet_size;
        self.num_packets += 1;

        if departure_time_us > self.departure_time_us {
            self.departure_time_us = departure_time_us;
        }
        if arrival_time_us > self.arrival_time_us {
            self.arrival_time_us = arrival_time_us;
        }
    }

    pub fn interarrival_time(&self, other: &PacketGroup) -> i64 {
        self.arrival_time_us
            .sub_assuming_small_delta(other.arrival_time_us)
    }

    pub fn interdeparture_time(&self, other: &PacketGroup) -> i64 {
        self.departure_time_us
            .sub_assuming_small_delta(other.departure_time_us)
    }
}
