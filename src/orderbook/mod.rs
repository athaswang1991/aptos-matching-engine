pub mod price;

use crate::error::{OrderBookError, Result};
use crate::types::{Order, Side, Trade};
use price::BuyPrice;
use rust_decimal::Decimal;
use std::collections::{BTreeMap, VecDeque};

pub struct OrderBook {
    buy_levels: BTreeMap<BuyPrice, VecDeque<Order>>,
    sell_levels: BTreeMap<Decimal, VecDeque<Order>>,
    sequence: u64,
    min_price: Decimal,
    max_price: Decimal,
    max_quantity: Decimal,
}

impl OrderBook {
    #[inline]
    pub fn new() -> Self {
        Self {
            buy_levels: BTreeMap::new(),
            sell_levels: BTreeMap::new(),
            sequence: 0,
            min_price: Decimal::from(1),
            max_price: Decimal::from(1_000_000),
            max_quantity: Decimal::from(1_000_000),
        }
    }

    pub fn place_order(
        &mut self,
        side: Side,
        price: Decimal,
        quantity: Decimal,
        id: u64,
    ) -> Result<Vec<Trade>> {
        if quantity <= Decimal::ZERO {
            return Err(OrderBookError::InvalidQuantity(
                "Quantity must be positive".to_string(),
            ));
        }

        if quantity > self.max_quantity {
            return Err(OrderBookError::InvalidQuantity(format!(
                "Quantity exceeds maximum: {}",
                self.max_quantity
            )));
        }

        if price < self.min_price || price > self.max_price {
            return Err(OrderBookError::InvalidPrice(format!(
                "Price must be between {} and {}",
                self.min_price, self.max_price
            )));
        }

        let timestamp = self.sequence;
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| OrderBookError::OverflowError("Sequence overflow".to_string()))?;

        match side {
            Side::Buy => self.place_buy_order(price, quantity, id, timestamp),
            Side::Sell => self.place_sell_order(price, quantity, id, timestamp),
        }
    }

    #[inline]
    fn place_buy_order(
        &mut self,
        price: Decimal,
        quantity: Decimal,
        id: u64,
        timestamp: u64,
    ) -> Result<Vec<Trade>> {
        let mut trades = Vec::new();
        let mut remaining = quantity;
        let mut exhausted_levels = Vec::new();

        for (&level_price, level_orders) in &mut self.sell_levels {
            if level_price > price {
                break;
            }

            remaining =
                Self::match_at_level(level_orders, remaining, level_price, id, &mut trades)?;

            if level_orders.is_empty() {
                exhausted_levels.push(level_price);
            }

            if remaining == Decimal::ZERO {
                break;
            }
        }

        for level in exhausted_levels {
            self.sell_levels.remove(&level);
        }

        if remaining > Decimal::ZERO {
            self.buy_levels
                .entry(BuyPrice(price))
                .or_default()
                .push_back(Order {
                    id,
                    quantity: remaining,
                    timestamp,
                });
        }

        Ok(trades)
    }

    #[inline]
    fn place_sell_order(
        &mut self,
        price: Decimal,
        quantity: Decimal,
        id: u64,
        timestamp: u64,
    ) -> Result<Vec<Trade>> {
        let mut trades = Vec::new();
        let mut remaining = quantity;
        let mut exhausted_levels = Vec::new();

        for (&BuyPrice(level_price), level_orders) in &mut self.buy_levels {
            if level_price < price {
                break;
            }

            remaining =
                Self::match_at_level(level_orders, remaining, level_price, id, &mut trades)?;

            if level_orders.is_empty() {
                exhausted_levels.push(BuyPrice(level_price));
            }

            if remaining == Decimal::ZERO {
                break;
            }
        }

        for level in exhausted_levels {
            self.buy_levels.remove(&level);
        }

        if remaining > Decimal::ZERO {
            self.sell_levels.entry(price).or_default().push_back(Order {
                id,
                quantity: remaining,
                timestamp,
            });
        }

        Ok(trades)
    }

    #[inline]
    fn match_at_level(
        level_orders: &mut VecDeque<Order>,
        mut remaining: Decimal,
        price: Decimal,
        taker_id: u64,
        trades: &mut Vec<Trade>,
    ) -> Result<Decimal> {
        while remaining > Decimal::ZERO && !level_orders.is_empty() {
            let maker_order = level_orders.front_mut().unwrap();
            let fill_quantity = remaining.min(maker_order.quantity);

            trades.push(Trade {
                price,
                quantity: fill_quantity,
                maker_id: maker_order.id,
                taker_id,
            });

            remaining = remaining
                .checked_sub(fill_quantity)
                .ok_or_else(|| OrderBookError::OverflowError("Quantity underflow".to_string()))?;
            maker_order.quantity = maker_order
                .quantity
                .checked_sub(fill_quantity)
                .ok_or_else(|| OrderBookError::OverflowError("Quantity underflow".to_string()))?;

            if maker_order.quantity == Decimal::ZERO {
                level_orders.pop_front();
            }
        }

        Ok(remaining)
    }

    #[inline]
    pub fn best_buy(&self) -> Option<(Decimal, Decimal)> {
        self.buy_levels
            .first_key_value()
            .map(|(BuyPrice(price), orders)| {
                let total_quantity: Decimal = orders.iter().map(|o| o.quantity).sum();
                (*price, total_quantity)
            })
    }

    #[inline]
    pub fn best_sell(&self) -> Option<(Decimal, Decimal)> {
        self.sell_levels.first_key_value().map(|(price, orders)| {
            let total_quantity: Decimal = orders.iter().map(|o| o.quantity).sum();
            (*price, total_quantity)
        })
    }

    #[inline]
    pub fn buy_depth(&self) -> usize {
        self.buy_levels.len()
    }

    #[inline]
    pub fn sell_depth(&self) -> usize {
        self.sell_levels.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buy_levels.is_empty() && self.sell_levels.is_empty()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.buy_levels.clear();
        self.sell_levels.clear();
    }

    #[inline]
    pub fn buy_levels(&self, limit: usize) -> Vec<(Decimal, Decimal)> {
        self.buy_levels
            .iter()
            .take(limit)
            .map(|(BuyPrice(price), orders)| {
                let total_quantity: Decimal = orders.iter().map(|o| o.quantity).sum();
                (*price, total_quantity)
            })
            .collect()
    }

    #[inline]
    pub fn sell_levels(&self, limit: usize) -> Vec<(Decimal, Decimal)> {
        self.sell_levels
            .iter()
            .take(limit)
            .map(|(price, orders)| {
                let total_quantity: Decimal = orders.iter().map(|o| o.quantity).sum();
                (*price, total_quantity)
            })
            .collect()
    }
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_empty_book() {
        let book = OrderBook::new();
        assert!(book.is_empty());
        assert_eq!(book.best_buy(), None);
        assert_eq!(book.best_sell(), None);
        assert_eq!(book.buy_depth(), 0);
        assert_eq!(book.sell_depth(), 0);
    }

    #[test]
    fn test_place_buy_order_no_match() {
        let mut book = OrderBook::new();
        let trades = book.place_order(Side::Buy, dec!(100), dec!(10), 1).unwrap();
        assert!(trades.is_empty());
        assert_eq!(book.best_buy(), Some((dec!(100), dec!(10))));
        assert_eq!(book.best_sell(), None);
        assert!(!book.is_empty());
    }

    #[test]
    fn test_place_sell_order_no_match() {
        let mut book = OrderBook::new();
        let trades = book
            .place_order(Side::Sell, dec!(100), dec!(10), 1)
            .unwrap();
        assert!(trades.is_empty());
        assert_eq!(book.best_buy(), None);
        assert_eq!(book.best_sell(), Some((dec!(100), dec!(10))));
    }

    #[test]
    fn test_full_match() {
        let mut book = OrderBook::new();
        book.place_order(Side::Buy, dec!(100), dec!(10), 1).unwrap();
        let trades = book
            .place_order(Side::Sell, dec!(100), dec!(10), 2)
            .unwrap();

        assert_eq!(trades.len(), 1);
        assert_eq!(
            trades[0],
            Trade {
                price: dec!(100),
                quantity: dec!(10),
                maker_id: 1,
                taker_id: 2,
            }
        );

        assert!(book.is_empty());
    }

    #[test]
    fn test_partial_fill() {
        let mut book = OrderBook::new();
        book.place_order(Side::Buy, dec!(100), dec!(10), 1).unwrap();
        let trades = book.place_order(Side::Sell, dec!(100), dec!(5), 2).unwrap();

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, dec!(5));
        assert_eq!(book.best_buy(), Some((dec!(100), dec!(5))));
        assert_eq!(book.best_sell(), None);
    }

    #[test]
    fn test_multiple_price_levels() {
        let mut book = OrderBook::new();
        book.place_order(Side::Buy, dec!(99), dec!(10), 1).unwrap();
        book.place_order(Side::Buy, dec!(100), dec!(10), 2).unwrap();
        book.place_order(Side::Buy, dec!(101), dec!(10), 3).unwrap();

        assert_eq!(book.best_buy(), Some((dec!(101), dec!(10))));

        let trades = book.place_order(Side::Sell, dec!(99), dec!(25), 4).unwrap();

        assert_eq!(trades.len(), 3);
        assert_eq!(trades[0].price, dec!(101));
        assert_eq!(trades[0].quantity, dec!(10));
        assert_eq!(trades[1].price, dec!(100));
        assert_eq!(trades[1].quantity, dec!(10));
        assert_eq!(trades[2].price, dec!(99));
        assert_eq!(trades[2].quantity, dec!(5));

        assert_eq!(book.best_buy(), Some((dec!(99), dec!(5))));
        assert_eq!(book.best_sell(), None);
    }

    #[test]
    fn test_price_time_priority() {
        let mut book = OrderBook::new();
        book.place_order(Side::Buy, dec!(100), dec!(10), 1).unwrap();
        book.place_order(Side::Buy, dec!(100), dec!(10), 2).unwrap();
        book.place_order(Side::Buy, dec!(100), dec!(10), 3).unwrap();

        assert_eq!(book.best_buy(), Some((dec!(100), dec!(30))));

        let trades = book
            .place_order(Side::Sell, dec!(100), dec!(25), 4)
            .unwrap();

        assert_eq!(trades.len(), 3);
        assert_eq!(trades[0].maker_id, 1);
        assert_eq!(trades[0].quantity, dec!(10));
        assert_eq!(trades[1].maker_id, 2);
        assert_eq!(trades[1].quantity, dec!(10));
        assert_eq!(trades[2].maker_id, 3);
        assert_eq!(trades[2].quantity, dec!(5));

        assert_eq!(book.best_buy(), Some((dec!(100), dec!(5))));
    }

    #[test]
    fn test_remainder_added_to_book() {
        let mut book = OrderBook::new();
        book.place_order(Side::Buy, dec!(100), dec!(10), 1).unwrap();
        let trades = book
            .place_order(Side::Sell, dec!(101), dec!(20), 2)
            .unwrap();

        assert!(trades.is_empty());
        assert_eq!(book.best_buy(), Some((dec!(100), dec!(10))));
        assert_eq!(book.best_sell(), Some((dec!(101), dec!(20))));
    }

    #[test]
    fn test_aggressive_buy_matches_multiple_sells() {
        let mut book = OrderBook::new();
        book.place_order(Side::Sell, dec!(100), dec!(10), 1)
            .unwrap();
        book.place_order(Side::Sell, dec!(101), dec!(10), 2)
            .unwrap();
        book.place_order(Side::Sell, dec!(102), dec!(10), 3)
            .unwrap();

        assert_eq!(book.best_sell(), Some((dec!(100), dec!(10))));

        let trades = book.place_order(Side::Buy, dec!(102), dec!(25), 4).unwrap();

        assert_eq!(trades.len(), 3);
        assert_eq!(trades[0].price, dec!(100));
        assert_eq!(trades[1].price, dec!(101));
        assert_eq!(trades[2].price, dec!(102));
        assert_eq!(trades[2].quantity, dec!(5));

        assert_eq!(book.best_sell(), Some((dec!(102), dec!(5))));
        assert_eq!(book.best_buy(), None);
    }

    #[test]
    fn test_zero_quantity_order() {
        let mut book = OrderBook::new();
        let result = book.place_order(Side::Buy, dec!(100), dec!(0), 1);
        assert!(result.is_err());
        assert!(book.is_empty());
    }

    #[test]
    fn test_clear_book() {
        let mut book = OrderBook::new();
        book.place_order(Side::Buy, dec!(100), dec!(10), 1).unwrap();
        book.place_order(Side::Sell, dec!(101), dec!(10), 2)
            .unwrap();

        assert!(!book.is_empty());
        book.clear();
        assert!(book.is_empty());
    }

    #[test]
    fn test_trade_at_maker_price() {
        let mut book = OrderBook::new();
        book.place_order(Side::Buy, dec!(102), dec!(10), 1).unwrap();

        let trades = book
            .place_order(Side::Sell, dec!(100), dec!(10), 2)
            .unwrap();

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].price, dec!(102));
        assert_eq!(trades[0].maker_id, 1);
        assert_eq!(trades[0].taker_id, 2);
    }

    #[test]
    fn test_decimal_precision() {
        let mut book = OrderBook::new();
        book.place_order(Side::Buy, dec!(100.50), dec!(10.25), 1)
            .unwrap();
        let trades = book
            .place_order(Side::Sell, dec!(100.25), dec!(5.125), 2)
            .unwrap();

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].price, dec!(100.50));
        assert_eq!(trades[0].quantity, dec!(5.125));
        assert_eq!(book.best_buy(), Some((dec!(100.50), dec!(5.125))));
    }
}
