pub mod metrics;
pub mod scenarios;

use aptos_matching_engine::types::Side;
use rand::Rng;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use scenarios::MarketScenario;

pub use metrics::LatencyMetrics;

pub struct MarketSimulator {
    next_order_id: u64,
    mid_price: Decimal,
    volatility: Decimal,
    pub scenario: MarketScenario,
    scenario_timer: u32,
}

impl MarketSimulator {
    pub fn new() -> Self {
        Self {
            next_order_id: 1,
            mid_price: dec!(1000),
            volatility: dec!(0.5),
            scenario: MarketScenario::Normal,
            scenario_timer: 0,
        }
    }

    fn update_scenario(&mut self) {
        let mut rng = rand::thread_rng();
        self.scenario_timer = self.scenario_timer.saturating_sub(1);

        if self.scenario_timer == 0 {
            self.scenario = match rng.gen_range(0..100) {
                0..=60 => MarketScenario::Normal,
                61..=75 => MarketScenario::HighVolatility,
                76..=85 => MarketScenario::FlashCrash,
                86..=95 => MarketScenario::Recovery,
                _ => MarketScenario::LiquidityCrisis,
            };
            self.scenario_timer = rng.gen_range(10..30);
        }
    }

    pub fn generate_order(&mut self) -> (Side, Decimal, Decimal, u64) {
        let mut rng = rand::thread_rng();
        self.update_scenario();

        let (volatility_mult, aggressive_prob, size_mult) = match self.scenario {
            MarketScenario::Normal => (dec!(1), 0.3, dec!(1)),
            MarketScenario::HighVolatility => (dec!(3), 0.5, dec!(1.5)),
            MarketScenario::FlashCrash => (dec!(10), 0.8, dec!(2)),
            MarketScenario::Recovery => (dec!(0.5), 0.2, dec!(0.8)),
            MarketScenario::LiquidityCrisis => (dec!(5), 0.1, dec!(0.3)),
        };

        let price_change =
            Decimal::from(rng.gen_range(-10..=10)) * self.volatility * volatility_mult / dec!(10);
        self.mid_price += price_change;
        self.mid_price = self.mid_price.max(dec!(900)).min(dec!(1100));

        let is_aggressive = rng.gen_bool(aggressive_prob);
        let side = if rng.gen_bool(0.5) {
            Side::Buy
        } else {
            Side::Sell
        };

        let price = if is_aggressive {
            match side {
                Side::Buy => self.mid_price + Decimal::from(rng.gen_range(5..15)),
                Side::Sell => self.mid_price - Decimal::from(rng.gen_range(5..15)),
            }
        } else {
            match side {
                Side::Buy => self.mid_price - Decimal::from(rng.gen_range(0..5)),
                Side::Sell => self.mid_price + Decimal::from(rng.gen_range(0..5)),
            }
        };

        let base_size = Decimal::from(rng.gen_range(50..200));
        let quantity = (base_size * size_mult).round();
        let id = self.next_order_id;
        self.next_order_id += 1;

        (side, price, quantity, id)
    }

    pub fn mid_price(&self) -> Decimal {
        self.mid_price
    }
}
