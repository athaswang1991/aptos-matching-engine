pub mod perps;

use std::cmp::Ordering;
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Order {
    id: u64,
    quantity: u64,
    timestamp: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trade {
    pub price: u64,
    pub quantity: u64,
    pub maker_id: u64,
    pub taker_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BuyPrice(u64);

impl PartialOrd for BuyPrice {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BuyPrice {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.cmp(&self.0)
    }
}

pub struct OrderBook {
    buy_levels: BTreeMap<BuyPrice, VecDeque<Order>>,
    sell_levels: BTreeMap<u64, VecDeque<Order>>,
    sequence: u64,
}

impl OrderBook {
    #[inline]
    pub fn new() -> Self {
        Self {
            buy_levels: BTreeMap::new(),
            sell_levels: BTreeMap::new(),
            sequence: 0,
        }
    }

    pub fn place_order(&mut self, side: Side, price: u64, quantity: u64, id: u64) -> Vec<Trade> {
        if quantity == 0 {
            return Vec::new();
        }

        let timestamp = self.sequence;
        self.sequence += 1;

        match side {
            Side::Buy => self.place_buy_order(price, quantity, id, timestamp),
            Side::Sell => self.place_sell_order(price, quantity, id, timestamp),
        }
    }

    #[inline]
    fn place_buy_order(
        &mut self,
        price: u64,
        quantity: u64,
        id: u64,
        timestamp: u64,
    ) -> Vec<Trade> {
        let mut trades = Vec::new();
        let mut remaining = quantity;
        let mut exhausted_levels = Vec::new();

        for (&level_price, level_orders) in &mut self.sell_levels {
            if level_price > price {
                break;
            }

            remaining = Self::match_at_level(level_orders, remaining, level_price, id, &mut trades);

            if level_orders.is_empty() {
                exhausted_levels.push(level_price);
            }

            if remaining == 0 {
                break;
            }
        }

        for level in exhausted_levels {
            self.sell_levels.remove(&level);
        }

        if remaining > 0 {
            self.buy_levels
                .entry(BuyPrice(price))
                .or_default()
                .push_back(Order {
                    id,
                    quantity: remaining,
                    timestamp,
                });
        }

        trades
    }

    #[inline]
    fn place_sell_order(
        &mut self,
        price: u64,
        quantity: u64,
        id: u64,
        timestamp: u64,
    ) -> Vec<Trade> {
        let mut trades = Vec::new();
        let mut remaining = quantity;
        let mut exhausted_levels = Vec::new();

        for (&BuyPrice(level_price), level_orders) in &mut self.buy_levels {
            if level_price < price {
                break;
            }

            remaining = Self::match_at_level(level_orders, remaining, level_price, id, &mut trades);

            if level_orders.is_empty() {
                exhausted_levels.push(BuyPrice(level_price));
            }

            if remaining == 0 {
                break;
            }
        }

        for level in exhausted_levels {
            self.buy_levels.remove(&level);
        }

        if remaining > 0 {
            self.sell_levels.entry(price).or_default().push_back(Order {
                id,
                quantity: remaining,
                timestamp,
            });
        }

        trades
    }

    #[inline]
    fn match_at_level(
        level_orders: &mut VecDeque<Order>,
        mut remaining: u64,
        price: u64,
        taker_id: u64,
        trades: &mut Vec<Trade>,
    ) -> u64 {
        while remaining > 0 && !level_orders.is_empty() {
            let maker_order = level_orders.front_mut().unwrap();
            let fill_quantity = remaining.min(maker_order.quantity);

            trades.push(Trade {
                price,
                quantity: fill_quantity,
                maker_id: maker_order.id,
                taker_id,
            });

            remaining -= fill_quantity;
            maker_order.quantity -= fill_quantity;

            if maker_order.quantity == 0 {
                level_orders.pop_front();
            }
        }

        remaining
    }

    #[inline]
    pub fn best_buy(&self) -> Option<(u64, u64)> {
        self.buy_levels
            .first_key_value()
            .map(|(BuyPrice(price), orders)| {
                let total_quantity: u64 = orders.iter().map(|o| o.quantity).sum();
                (*price, total_quantity)
            })
    }

    #[inline]
    pub fn best_sell(&self) -> Option<(u64, u64)> {
        self.sell_levels.first_key_value().map(|(price, orders)| {
            let total_quantity: u64 = orders.iter().map(|o| o.quantity).sum();
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
    pub fn buy_levels(&self, limit: usize) -> Vec<(u64, u64)> {
        self.buy_levels
            .iter()
            .take(limit)
            .map(|(BuyPrice(price), orders)| {
                let total_quantity: u64 = orders.iter().map(|o| o.quantity).sum();
                (*price, total_quantity)
            })
            .collect()
    }

    #[inline]
    pub fn sell_levels(&self, limit: usize) -> Vec<(u64, u64)> {
        self.sell_levels
            .iter()
            .take(limit)
            .map(|(price, orders)| {
                let total_quantity: u64 = orders.iter().map(|o| o.quantity).sum();
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
        let trades = book.place_order(Side::Buy, 100, 10, 1);
        assert!(trades.is_empty());
        assert_eq!(book.best_buy(), Some((100, 10)));
        assert_eq!(book.best_sell(), None);
        assert!(!book.is_empty());
    }

    #[test]
    fn test_place_sell_order_no_match() {
        let mut book = OrderBook::new();
        let trades = book.place_order(Side::Sell, 100, 10, 1);
        assert!(trades.is_empty());
        assert_eq!(book.best_buy(), None);
        assert_eq!(book.best_sell(), Some((100, 10)));
    }

    #[test]
    fn test_full_match() {
        let mut book = OrderBook::new();
        book.place_order(Side::Buy, 100, 10, 1);
        let trades = book.place_order(Side::Sell, 100, 10, 2);

        assert_eq!(trades.len(), 1);
        assert_eq!(
            trades[0],
            Trade {
                price: 100,
                quantity: 10,
                maker_id: 1,
                taker_id: 2,
            }
        );

        assert!(book.is_empty());
    }

    #[test]
    fn test_partial_fill() {
        let mut book = OrderBook::new();
        book.place_order(Side::Buy, 100, 10, 1);
        let trades = book.place_order(Side::Sell, 100, 5, 2);

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].quantity, 5);
        assert_eq!(book.best_buy(), Some((100, 5)));
        assert_eq!(book.best_sell(), None);
    }

    #[test]
    fn test_multiple_price_levels() {
        let mut book = OrderBook::new();
        book.place_order(Side::Buy, 99, 10, 1);
        book.place_order(Side::Buy, 100, 10, 2);
        book.place_order(Side::Buy, 101, 10, 3);

        assert_eq!(book.best_buy(), Some((101, 10)));

        let trades = book.place_order(Side::Sell, 99, 25, 4);

        assert_eq!(trades.len(), 3);
        assert_eq!(trades[0].price, 101);
        assert_eq!(trades[0].quantity, 10);
        assert_eq!(trades[1].price, 100);
        assert_eq!(trades[1].quantity, 10);
        assert_eq!(trades[2].price, 99);
        assert_eq!(trades[2].quantity, 5);

        assert_eq!(book.best_buy(), Some((99, 5)));
        assert_eq!(book.best_sell(), None);
    }

    #[test]
    fn test_price_time_priority() {
        let mut book = OrderBook::new();
        book.place_order(Side::Buy, 100, 10, 1);
        book.place_order(Side::Buy, 100, 10, 2);
        book.place_order(Side::Buy, 100, 10, 3);

        assert_eq!(book.best_buy(), Some((100, 30)));

        let trades = book.place_order(Side::Sell, 100, 25, 4);

        assert_eq!(trades.len(), 3);
        assert_eq!(trades[0].maker_id, 1);
        assert_eq!(trades[0].quantity, 10);
        assert_eq!(trades[1].maker_id, 2);
        assert_eq!(trades[1].quantity, 10);
        assert_eq!(trades[2].maker_id, 3);
        assert_eq!(trades[2].quantity, 5);

        assert_eq!(book.best_buy(), Some((100, 5)));
    }

    #[test]
    fn test_remainder_added_to_book() {
        let mut book = OrderBook::new();
        book.place_order(Side::Buy, 100, 10, 1);
        let trades = book.place_order(Side::Sell, 101, 20, 2);

        assert!(trades.is_empty());
        assert_eq!(book.best_buy(), Some((100, 10)));
        assert_eq!(book.best_sell(), Some((101, 20)));
    }

    #[test]
    fn test_aggressive_buy_matches_multiple_sells() {
        let mut book = OrderBook::new();
        book.place_order(Side::Sell, 100, 10, 1);
        book.place_order(Side::Sell, 101, 10, 2);
        book.place_order(Side::Sell, 102, 10, 3);

        assert_eq!(book.best_sell(), Some((100, 10)));

        let trades = book.place_order(Side::Buy, 102, 25, 4);

        assert_eq!(trades.len(), 3);
        assert_eq!(trades[0].price, 100);
        assert_eq!(trades[1].price, 101);
        assert_eq!(trades[2].price, 102);
        assert_eq!(trades[2].quantity, 5);

        assert_eq!(book.best_sell(), Some((102, 5)));
        assert_eq!(book.best_buy(), None);
    }

    #[test]
    fn test_zero_quantity_order() {
        let mut book = OrderBook::new();
        let trades = book.place_order(Side::Buy, 100, 0, 1);
        assert!(trades.is_empty());
        assert!(book.is_empty());
    }

    #[test]
    fn test_clear_book() {
        let mut book = OrderBook::new();
        book.place_order(Side::Buy, 100, 10, 1);
        book.place_order(Side::Sell, 101, 10, 2);

        assert!(!book.is_empty());
        book.clear();
        assert!(book.is_empty());
    }

    #[test]
    fn test_trade_at_maker_price() {
        let mut book = OrderBook::new();
        book.place_order(Side::Buy, 102, 10, 1);

        let trades = book.place_order(Side::Sell, 100, 10, 2);

        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].price, 102);
        assert_eq!(trades[0].maker_id, 1);
        assert_eq!(trades[0].taker_id, 2);
    }
}
