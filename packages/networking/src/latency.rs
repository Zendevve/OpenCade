use std::collections::VecDeque;

/// EWMA alpha for latency smoothing.
const ALPHA: f64 = 0.2;
const RING_CAP: usize = 30;

/// Latency and link quality metrics.
#[derive(Debug, Clone)]
pub struct LatencyMetrics {
    /// EWMA of RTT in ms, None until first sample.
    pub rtt_ms: Option<f64>,
    /// Packet loss fraction 0.0..1.0
    pub loss: f32,
    /// EWMA of jitter (inter-arrival) in ms
    pub jitter_ms: f32,
    samples: VecDeque<u64>,
}

impl Default for LatencyMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl LatencyMetrics {
    pub fn new() -> Self {
        Self {
            rtt_ms: None,
            loss: 0.0,
            jitter_ms: 0.0,
            samples: VecDeque::with_capacity(RING_CAP),
        }
    }

    /// Record a round-trip time sample in milliseconds.
    pub fn record_rtt(&mut self, sample_ms: u64) {
        let sample = sample_ms as f64;
        self.rtt_ms = Some(match self.rtt_ms {
            Some(prev) => prev * (1.0 - ALPHA) + sample * ALPHA,
            None => sample,
        });
        if self.samples.len() == RING_CAP {
            self.samples.pop_front();
        }
        self.samples.push_back(sample_ms);
    }

    /// Record loss given sent and acked counts.
    pub fn record_loss(&mut self, sent: u32, acked: u32) {
        if sent == 0 {
            self.loss = 0.0;
            return;
        }
        let lost = sent.saturating_sub(acked);
        let fraction = lost as f32 / sent as f32;
        self.loss = fraction.clamp(0.0, 1.0);
    }

    /// Record inter-arrival jitter.
    pub fn record_jitter(&mut self, inter_arrival_ms: u64) {
        let sample = inter_arrival_ms as f32;
        if self.jitter_ms == 0.0 {
            self.jitter_ms = sample;
        } else {
            self.jitter_ms = self.jitter_ms * 0.8 + sample * 0.2;
        }
    }

    /// Snapshot returning p50-like EWMA as u64 and loss/jitter.
    pub fn snapshot(&self) -> (Option<u64>, f32, f32) {
        let rtt = self.rtt_ms.map(|v| v.round() as u64);
        (rtt, self.loss, self.jitter_ms)
    }

    /// Number of samples stored (for testing).
    #[cfg(test)]
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ewma_alpha_0_2() {
        let mut m = LatencyMetrics::new();
        m.record_rtt(100);
        assert_eq!(m.snapshot().0, Some(100));
        m.record_rtt(200);
        // 100*0.8 + 200*0.2 = 120
        assert_eq!(m.snapshot().0, Some(120));
        m.record_rtt(300);
        // 120*0.8 + 300*0.2 = 96+60=156
        assert_eq!(m.snapshot().0, Some(156));
        // Check floating value directly
        assert!((m.rtt_ms.unwrap() - 156.0).abs() < 1e-6);
    }

    #[test]
    fn loss_calculation() {
        let mut m = LatencyMetrics::new();
        m.record_loss(10, 8);
        assert!((m.loss - 0.2).abs() < 1e-6);
        assert_eq!(m.snapshot().1, m.loss);

        m.record_loss(4, 4);
        assert!((m.loss - 0.0).abs() < 1e-6);

        m.record_loss(0, 0);
        assert_eq!(m.loss, 0.0);

        m.record_loss(10, 0);
        assert!((m.loss - 1.0).abs() < 1e-6);

        // Clamp check: acked > sent would saturate to 0 lost
        m.record_loss(5, 10);
        assert_eq!(m.loss, 0.0);
    }

    #[test]
    fn jitter_ewma() {
        let mut m = LatencyMetrics::new();
        m.record_jitter(10);
        assert!((m.jitter_ms - 10.0).abs() < 1e-6);
        m.record_jitter(20);
        // 10*0.8+20*0.2=12
        assert!((m.jitter_ms - 12.0).abs() < 1e-6);
        m.record_jitter(30);
        // 12*0.8+30*0.2=9.6+6=15.6
        assert!((m.jitter_ms - 15.6).abs() < 1e-3);
        assert!((m.snapshot().2 - 15.6).abs() < 1e-3);
    }

    #[test]
    fn ring_buffer_cap_30() {
        let mut m = LatencyMetrics::new();
        for i in 0..40 {
            m.record_rtt(i);
        }
        assert_eq!(m.sample_count(), 30);
        // first 10 should have been evicted, remaining are 10..39
        assert_eq!(*m.samples.front().unwrap(), 10);
        assert_eq!(*m.samples.back().unwrap(), 39);
    }

    #[test]
    fn snapshot_returns_correct_tuple() {
        let mut m = LatencyMetrics::new();
        assert_eq!(m.snapshot(), (None, 0.0, 0.0));
        m.record_rtt(50);
        m.record_loss(10, 9);
        m.record_jitter(5);
        let (rtt, loss, jitter) = m.snapshot();
        assert_eq!(rtt, Some(50));
        assert!((loss - 0.1).abs() < 1e-6);
        assert!((jitter - 5.0).abs() < 1e-6);
    }
}
