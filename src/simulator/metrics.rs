use std::collections::VecDeque;
use std::time::Duration;

const LATENCY_HISTORY_SIZE: usize = 100;

pub struct LatencyMetrics {
    pub execution_latencies: VecDeque<Duration>,
    pub datafeed_latencies: VecDeque<Duration>,
    pub last_execution: Option<Duration>,
    pub avg_execution: Duration,
    pub avg_datafeed: Duration,
    pub p99_execution: Duration,
    pub p99_datafeed: Duration,
}

impl LatencyMetrics {
    pub fn new() -> Self {
        Self {
            execution_latencies: VecDeque::new(),
            datafeed_latencies: VecDeque::new(),
            last_execution: None,
            avg_execution: Duration::ZERO,
            avg_datafeed: Duration::ZERO,
            p99_execution: Duration::ZERO,
            p99_datafeed: Duration::ZERO,
        }
    }

    pub fn record_execution(&mut self, latency: Duration) {
        self.last_execution = Some(latency);
        self.execution_latencies.push_back(latency);

        if self.execution_latencies.len() > LATENCY_HISTORY_SIZE {
            self.execution_latencies.pop_front();
        }

        self.update_stats();
    }

    #[allow(dead_code)]
    pub fn record_datafeed(&mut self, latency: Duration) {
        self.datafeed_latencies.push_back(latency);

        if self.datafeed_latencies.len() > LATENCY_HISTORY_SIZE {
            self.datafeed_latencies.pop_front();
        }

        self.update_stats();
    }

    fn update_stats(&mut self) {
        if !self.execution_latencies.is_empty() {
            let sum: Duration = self.execution_latencies.iter().sum();
            self.avg_execution = sum / self.execution_latencies.len() as u32;

            let mut sorted: Vec<Duration> = self.execution_latencies.iter().cloned().collect();
            sorted.sort();
            let p99_idx = (sorted.len() as f64 * 0.99) as usize;
            self.p99_execution = sorted.get(p99_idx).cloned().unwrap_or(Duration::ZERO);
        }

        if !self.datafeed_latencies.is_empty() {
            let sum: Duration = self.datafeed_latencies.iter().sum();
            self.avg_datafeed = sum / self.datafeed_latencies.len() as u32;

            let mut sorted: Vec<Duration> = self.datafeed_latencies.iter().cloned().collect();
            sorted.sort();
            let p99_idx = (sorted.len() as f64 * 0.99) as usize;
            self.p99_datafeed = sorted.get(p99_idx).cloned().unwrap_or(Duration::ZERO);
        }
    }
}
