pub mod error;
pub mod funding;
pub mod orderbook;
pub mod perps;
pub mod types;

// Core exports
pub use error::{OrderBookError, Result};
pub use orderbook::OrderBook;
pub use types::{Side, Trade};

// Funding exports
pub use funding::FundingRate;

// Perps exports - re-export everything from perps module
pub use perps::*;
