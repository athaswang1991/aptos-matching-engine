use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum OrderBookError {
    #[error("Invalid order quantity: {0}")]
    InvalidQuantity(String),

    #[error("Invalid price: {0}")]
    InvalidPrice(String),

    #[error("Order not found: {id}")]
    OrderNotFound { id: u64 },

    #[error("Insufficient margin: required {required}, provided {provided}")]
    InsufficientMargin { required: u64, provided: u64 },

    #[error("Position not found for trader: {trader_id}")]
    PositionNotFound { trader_id: u64 },

    #[error("Invalid leverage: {0}")]
    InvalidLeverage(f64),

    #[error("Market manipulation detected: {0}")]
    MarketManipulation(String),

    #[error("Overflow in calculation: {0}")]
    OverflowError(String),
}

pub type Result<T> = std::result::Result<T, OrderBookError>;
