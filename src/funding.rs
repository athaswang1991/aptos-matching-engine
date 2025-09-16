use crate::error::Result;
use rust_decimal::Decimal;
use std::collections::VecDeque;

const FUNDING_INTERVAL_SECONDS: u64 = 28800;
const MAX_FUNDING_RATE: Decimal = Decimal::ONE;
const MIN_FUNDING_RATE: Decimal = Decimal::NEGATIVE_ONE;
const INTEREST_RATE: Decimal = Decimal::ZERO;

#[derive(Debug, Clone)]
pub struct PriceSample {
    pub mark_price: Decimal,
    pub index_price: Decimal,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct FundingRate {
    pub current_rate: Decimal,
    pub next_funding_time: u64,
    pub premium_index: Decimal,
    pub interest_rate: Decimal,
    pub long_open_interest: Decimal,
    pub short_open_interest: Decimal,
    price_samples: VecDeque<PriceSample>,
    sample_interval: u64,
    max_samples: usize,
}

impl Default for FundingRate {
    fn default() -> Self {
        Self::new()
    }
}

impl FundingRate {
    pub fn new() -> Self {
        Self {
            current_rate: Decimal::ZERO,
            next_funding_time: FUNDING_INTERVAL_SECONDS,
            premium_index: Decimal::ZERO,
            interest_rate: INTEREST_RATE,
            long_open_interest: Decimal::ZERO,
            short_open_interest: Decimal::ZERO,
            price_samples: VecDeque::new(),
            sample_interval: 60,
            max_samples: 480,
        }
    }

    pub fn add_price_sample(&mut self, mark_price: Decimal, index_price: Decimal, timestamp: u64) {
        let sample = PriceSample {
            mark_price,
            index_price,
            timestamp,
        };

        self.price_samples.push_back(sample);

        if self.price_samples.len() > self.max_samples {
            self.price_samples.pop_front();
        }
    }

    pub fn calculate_twap_premium(&self, lookback_seconds: u64) -> Result<Decimal> {
        if self.price_samples.is_empty() {
            return Ok(Decimal::ZERO);
        }

        let current_time = self
            .price_samples
            .back()
            .map(|s| s.timestamp)
            .unwrap_or(0);
        let start_time = current_time.saturating_sub(lookback_seconds);

        let relevant_samples: Vec<&PriceSample> = self
            .price_samples
            .iter()
            .filter(|s| s.timestamp >= start_time)
            .collect();

        if relevant_samples.is_empty() {
            return Ok(Decimal::ZERO);
        }

        let mut weighted_premium = Decimal::ZERO;
        let mut total_weight = Decimal::ZERO;

        for i in 0..relevant_samples.len() {
            let sample = relevant_samples[i];
            let premium = (sample.mark_price - sample.index_price) / sample.index_price;

            let weight = if i < relevant_samples.len() - 1 {
                Decimal::from(relevant_samples[i + 1].timestamp - sample.timestamp)
            } else {
                Decimal::from(60)
            };

            weighted_premium += premium * weight;
            total_weight += weight;
        }

        if total_weight.is_zero() {
            Ok(Decimal::ZERO)
        } else {
            Ok(weighted_premium / total_weight)
        }
    }

    pub fn calculate_funding_rate(&mut self, timestamp: u64) -> Result<Decimal> {
        let premium_8h = self.calculate_twap_premium(FUNDING_INTERVAL_SECONDS)?;

        self.premium_index = premium_8h;

        let funding_rate = premium_8h + self.interest_rate;

        self.current_rate = funding_rate
            .max(MIN_FUNDING_RATE / Decimal::from(100))
            .min(MAX_FUNDING_RATE / Decimal::from(100));

        self.next_funding_time = timestamp + FUNDING_INTERVAL_SECONDS;

        Ok(self.current_rate)
    }

    pub fn update_open_interest(&mut self, long_oi: Decimal, short_oi: Decimal) {
        self.long_open_interest = long_oi;
        self.short_open_interest = short_oi;
    }

    pub fn get_imbalance_ratio(&self) -> Decimal {
        let total_oi = self.long_open_interest + self.short_open_interest;
        if total_oi.is_zero() {
            Decimal::ZERO
        } else {
            (self.long_open_interest - self.short_open_interest) / total_oi
        }
    }

    pub fn should_apply_funding(&self, timestamp: u64) -> bool {
        timestamp >= self.next_funding_time
    }

    pub fn calculate_funding_payment(
        &self,
        position_size: Decimal,
        is_long: bool,
    ) -> Decimal {
        let payment = position_size * self.current_rate;
        if is_long {
            -payment
        } else {
            payment
        }
    }
}