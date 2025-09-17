use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trade {
    pub price: Decimal,
    pub quantity: Decimal,
    pub maker_id: u64,
    pub taker_id: u64,
}
