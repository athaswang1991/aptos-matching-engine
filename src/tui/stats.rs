use rust_decimal::Decimal;

pub struct MarketStats {
    pub bid_volume: Decimal,
    pub ask_volume: Decimal,
    pub spread: Decimal,
    pub imbalance: f64,
}
