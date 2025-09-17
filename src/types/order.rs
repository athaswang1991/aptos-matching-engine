use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    pub id: u64,
    pub quantity: Decimal,
    pub timestamp: u64,
}
