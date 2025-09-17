use crate::simulator::{LatencyMetrics, MarketSimulator};
use crate::tui::stats::MarketStats;
use imlob::orderbook::OrderBook;
use imlob::types::{Side, Trade};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const MAX_TRADES: usize = 20;
const MAX_EVENTS: usize = 15;

pub struct App {
    pub order_book: OrderBook,
    pub trades: VecDeque<(Trade, Instant)>,
    pub events: VecDeque<(String, Instant)>,
    pub simulator: MarketSimulator,
    pub last_update: Instant,
    pub update_interval: Duration,
    pub total_trades: u64,
    pub total_volume: Decimal,
    pub paused: bool,
    pub price_history: VecDeque<Decimal>,
    pub last_trade_price: Option<Decimal>,
    pub last_trade_direction: Option<Side>,
    pub latency_metrics: LatencyMetrics,
    pub market_stats: MarketStats,
}

impl App {
    pub fn new() -> Self {
        Self {
            order_book: OrderBook::new(),
            trades: VecDeque::new(),
            events: VecDeque::new(),
            simulator: MarketSimulator::new(),
            last_update: Instant::now(),
            update_interval: Duration::from_millis(500),
            total_trades: 0,
            total_volume: Decimal::ZERO,
            paused: false,
            price_history: VecDeque::new(),
            last_trade_price: None,
            last_trade_direction: None,
            latency_metrics: LatencyMetrics::new(),
            market_stats: MarketStats {
                bid_volume: Decimal::ZERO,
                ask_volume: Decimal::ZERO,
                spread: Decimal::ZERO,
                imbalance: 0.0,
            },
        }
    }

    pub fn update_market_stats(&mut self) {
        let bid_levels = self.order_book.buy_levels(10);
        let ask_levels = self.order_book.sell_levels(10);

        self.market_stats.bid_volume = bid_levels.iter().map(|(_, qty)| *qty).sum();
        self.market_stats.ask_volume = ask_levels.iter().map(|(_, qty)| *qty).sum();

        if let (Some((bid, _)), Some((ask, _))) =
            (self.order_book.best_buy(), self.order_book.best_sell())
        {
            self.market_stats.spread = ask - bid;

            let total_vol = self.market_stats.bid_volume + self.market_stats.ask_volume;
            if total_vol > Decimal::ZERO {
                self.market_stats.imbalance =
                    ((self.market_stats.bid_volume - self.market_stats.ask_volume) / total_vol)
                        .to_f64()
                        .unwrap_or(0.0);
            }
        }
    }

    pub fn update(&mut self) {
        if self.paused || self.last_update.elapsed() < self.update_interval {
            return;
        }

        let start = Instant::now();
        let (side, price, quantity, id) = self.simulator.generate_order();

        let trades_result = self.order_book.place_order(side, price, quantity, id);

        match trades_result {
            Ok(trades) => {
                let latency = start.elapsed();
                self.latency_metrics.record_execution(latency);

                if trades.is_empty() {
                    self.events.push_front((
                        format!("{side:?} order #{id} added: {quantity} @ {price}"),
                        Instant::now(),
                    ));
                } else {
                    for trade in &trades {
                        self.total_trades += 1;
                        self.total_volume += trade.quantity;

                        self.trades.push_front((trade.clone(), Instant::now()));
                        if self.trades.len() > MAX_TRADES {
                            self.trades.pop_back();
                        }

                        self.last_trade_price = Some(trade.price);
                        self.last_trade_direction = Some(side);

                        self.price_history.push_front(trade.price);
                        if self.price_history.len() > 50 {
                            self.price_history.pop_back();
                        }
                    }

                    self.events.push_front((
                        format!("{} trade(s) executed", trades.len()),
                        Instant::now(),
                    ));
                }
            }
            Err(e) => {
                self.events
                    .push_front((format!("Order failed: {e}"), Instant::now()));
            }
        }

        if self.events.len() > MAX_EVENTS {
            self.events.pop_back();
        }

        self.update_market_stats();
        self.last_update = Instant::now();
    }

    pub fn get_book_levels(&self, side: Side, limit: usize) -> Vec<(Decimal, Decimal)> {
        match side {
            Side::Buy => self.order_book.buy_levels(limit),
            Side::Sell => self.order_book.sell_levels(limit),
        }
    }
}
