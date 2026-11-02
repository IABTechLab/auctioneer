//! Per-bidder `responsetimemillis` aggregation.

use std::time::Duration;

#[derive(Clone, Copy, Debug, Default)]
pub struct ResponseTimeStat {
    pub make_requests: Duration,
    pub max_call_elapsed: Duration,
    pub make_bids: Duration,
}

impl ResponseTimeStat {
    #[must_use]
    pub fn total_ms(&self) -> u32 {
        let total = self
            .make_requests
            .saturating_add(self.max_call_elapsed)
            .saturating_add(self.make_bids);
        u32::try_from(total.as_millis()).unwrap_or(u32::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::ResponseTimeStat;
    use std::time::Duration;

    #[test]
    fn total_sums_three_segments() {
        let stat = ResponseTimeStat {
            make_requests: Duration::from_millis(5),
            max_call_elapsed: Duration::from_millis(100),
            make_bids: Duration::from_millis(2),
        };
        assert_eq!(stat.total_ms(), 107);
    }
}
