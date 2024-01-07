use super::*;

struct AscendingMinima<T: PartialOrd, const N: u32> {
    window: VecDeque<(u32, T)>,
    counter: u32,
    capacity_reached: bool,
}

impl<T: PartialOrd, const N: u32> AscendingMinima<T, N> {
    fn new() -> AscendingMinima<T, N> {
        AscendingMinima {
            window: VecDeque::with_capacity((N + 1) as usize),
            counter: 0,
            capacity_reached: false,
        }
    }

    fn push(&mut self, value: T) {
        while let Some(item) = self.window.back() {
            if item.1 >= value {
                self.window.pop_back();
            } else {
                break;
            }
        }
        self.window.push_back((self.counter, value));

        if self.capacity_reached {
            if let Some(item) = self.window.front() {
                if self.counter.wrapping_sub(item.0) >= N {
                    self.window.pop_front();
                }
            }
            self.counter = self.counter.wrapping_add(1);
        } else {
            self.counter = self.counter.wrapping_add(1);
            if self.counter == N {
                self.capacity_reached = true;
            }
        }
    }

    fn minimum(&self) -> Option<&T> {
        self.window.front().map(|v| &v.1)
    }
}

struct WindowData {
    arrival_time_us: TwccTime,
    size_bytes: u64,
    num_packets: u64,
}

pub struct History {
    data: VecDeque<WindowData>,
    ascending_minima: AscendingMinima<i64, WINDOW_SIZE>,
    total_packet_size_bytes: u64,
    num_packets: u64,
}

impl History {
    pub fn new() -> History {
        History {
            data: VecDeque::new(),
            ascending_minima: AscendingMinima::new(),
            total_packet_size_bytes: 0,
            num_packets: 0,
        }
    }

    pub fn add_group(&mut self, curr_group: &PacketGroup, interdeparture_time: i64) {
        let window_data = WindowData {
            arrival_time_us: curr_group.arrival_time_us,
            size_bytes: curr_group.size_bytes,
            num_packets: curr_group.num_packets,
        };

        self.total_packet_size_bytes += window_data.size_bytes;
        self.num_packets += window_data.num_packets;
        self.data.push_back(window_data);
        self.ascending_minima.push(interdeparture_time);

        if self.data.len() > WINDOW_SIZE as usize {
            if let Some(to_remove) = self.data.pop_front() {
                self.total_packet_size_bytes -= to_remove.size_bytes;
                self.num_packets -= to_remove.num_packets;
            }
        }
    }

    pub fn average_packet_size_bytes(&self) -> f64 {
        self.total_packet_size_bytes as f64 / self.num_packets as f64
    }

    pub fn received_bandwidth_bytes_per_sec(&self) -> Option<f64> {
        let start = self.data.front()?.arrival_time_us;
        let end = self.data.back()?.arrival_time_us;
        let timespan = end.sub_assuming_small_delta(start);
        // Timespan is in microseconds so multiply by 1e6
        Some(1e6 * self.total_packet_size_bytes as f64 / timespan as f64)
    }

    /// Used for computing f_max in the arrival-time filter
    pub fn smallest_send_interval(&self) -> Option<&i64> {
        self.ascending_minima.minimum()
    }
}
