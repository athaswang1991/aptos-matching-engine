use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PositionSide {
    Long,
    Short,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub trader_id: u64,
    pub side: PositionSide,
    pub size: u64,
    pub entry_price: f64,
    pub margin: u64,
    pub leverage: f64,
    pub unrealized_pnl: f64,
    pub liquidation_price: f64,
}

#[derive(Debug, Clone)]
pub struct FundingRate {
    pub rate: f64,
    pub next_timestamp: u64,
    pub long_open_interest: u64,
    pub short_open_interest: u64,
    pub premium_index: f64,
}

impl Default for FundingRate {
    fn default() -> Self {
        Self::new()
    }
}

impl FundingRate {
    pub fn new() -> Self {
        Self {
            rate: 0.0,
            next_timestamp: 0,
            long_open_interest: 0,
            short_open_interest: 0,
            premium_index: 0.0,
        }
    }

    pub fn calculate_rate(&mut self, mark_price: f64, index_price: f64) {
        self.premium_index = (mark_price - index_price) / index_price;

        let base_rate = self.premium_index * 0.1;
        self.rate = base_rate.clamp(-0.001, 0.001);
    }

    pub fn update_open_interest(&mut self, long_oi: u64, short_oi: u64) {
        self.long_open_interest = long_oi;
        self.short_open_interest = short_oi;
    }
}

#[derive(Debug, Clone)]
pub struct OraclePrice {
    pub price: f64,
    pub timestamp: u64,
    pub confidence: f64,
    pub source: String,
}

impl OraclePrice {
    pub fn new(price: f64) -> Self {
        Self {
            price,
            timestamp: 0,
            confidence: 0.99,
            source: "Simulated".to_string(),
        }
    }

    pub fn update(&mut self, spot_price: f64) {
        let noise = (rand::random::<f64>() - 0.5) * 0.001;
        self.price = spot_price * (1.0 + noise);
        self.timestamp += 1;
    }
}

#[derive(Debug, Clone)]
pub struct MarkPrice {
    pub price: f64,
    pub fair_price: f64,
    pub index_price: f64,
    pub funding_basis: f64,
}

impl Default for MarkPrice {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkPrice {
    pub fn new() -> Self {
        Self {
            price: 1000.0,
            fair_price: 1000.0,
            index_price: 1000.0,
            funding_basis: 0.0,
        }
    }

    pub fn calculate(&mut self, best_bid: f64, best_ask: f64, index_price: f64) {
        self.fair_price = (best_bid + best_ask) / 2.0;
        self.index_price = index_price;

        let basis = self.fair_price - self.index_price;
        self.funding_basis = self.funding_basis * 0.9 + basis * 0.1;

        self.price = self.index_price + self.funding_basis;
    }
}

#[derive(Debug)]
pub struct LiquidationEngine {
    pub maintenance_margin: f64,
    pub initial_margin: f64,
    pub liquidation_fee: f64,
    pub insurance_fund: u64,
}

impl Default for LiquidationEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LiquidationEngine {
    pub fn new() -> Self {
        Self {
            maintenance_margin: 0.005,
            initial_margin: 0.01,
            liquidation_fee: 0.003,
            insurance_fund: 1000000,
        }
    }

    pub fn calculate_liquidation_price(&self, position: &Position) -> f64 {
        let margin_ratio = self.maintenance_margin + self.liquidation_fee;

        match position.side {
            PositionSide::Long => position.entry_price * (1.0 - margin_ratio * position.leverage),
            PositionSide::Short => position.entry_price * (1.0 + margin_ratio * position.leverage),
        }
    }

    pub fn should_liquidate(&self, position: &Position, mark_price: f64) -> bool {
        match position.side {
            PositionSide::Long => mark_price <= position.liquidation_price,
            PositionSide::Short => mark_price >= position.liquidation_price,
        }
    }

    pub fn calculate_pnl(position: &Position, mark_price: f64) -> f64 {
        let price_diff = mark_price - position.entry_price;
        match position.side {
            PositionSide::Long => price_diff * position.size as f64,
            PositionSide::Short => -price_diff * position.size as f64,
        }
    }

    pub fn calculate_margin_ratio(&self, position: &Position, mark_price: f64) -> f64 {
        let pnl = Self::calculate_pnl(position, mark_price);
        let position_value = mark_price * position.size as f64;
        (position.margin as f64 + pnl) / position_value
    }
}

#[derive(Debug)]
pub struct PositionManager {
    pub positions: HashMap<u64, Position>,
    pub total_long_interest: u64,
    pub total_short_interest: u64,
    pub max_leverage: f64,
}

impl Default for PositionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PositionManager {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
            total_long_interest: 0,
            total_short_interest: 0,
            max_leverage: 100.0,
        }
    }

    pub fn open_position(
        &mut self,
        trader_id: u64,
        side: PositionSide,
        size: u64,
        entry_price: f64,
        margin: u64,
        liquidation_engine: &LiquidationEngine,
    ) -> Position {
        let leverage = (entry_price * size as f64) / margin as f64;
        let leverage = leverage.min(self.max_leverage);

        let mut position = Position {
            trader_id,
            side,
            size,
            entry_price,
            margin,
            leverage,
            unrealized_pnl: 0.0,
            liquidation_price: 0.0,
        };

        position.liquidation_price = liquidation_engine.calculate_liquidation_price(&position);

        match side {
            PositionSide::Long => self.total_long_interest += size,
            PositionSide::Short => self.total_short_interest += size,
        }

        self.positions.insert(trader_id, position.clone());
        position
    }

    pub fn close_position(&mut self, trader_id: u64) -> Option<Position> {
        if let Some(position) = self.positions.remove(&trader_id) {
            match position.side {
                PositionSide::Long => self.total_long_interest -= position.size,
                PositionSide::Short => self.total_short_interest -= position.size,
            }
            Some(position)
        } else {
            None
        }
    }

    pub fn update_positions(
        &mut self,
        mark_price: f64,
        liquidation_engine: &LiquidationEngine,
    ) -> Vec<u64> {
        let mut liquidated = Vec::new();

        for (trader_id, position) in self.positions.iter_mut() {
            position.unrealized_pnl = LiquidationEngine::calculate_pnl(position, mark_price);

            if liquidation_engine.should_liquidate(position, mark_price) {
                liquidated.push(*trader_id);
            }
        }

        for trader_id in &liquidated {
            self.close_position(*trader_id);
        }

        liquidated
    }
}

#[derive(Debug, Clone)]
pub struct FeeStructure {
    pub maker_fee: f64,
    pub taker_fee: f64,
    pub liquidation_fee: f64,
    pub funding_interval: u64,
}

impl Default for FeeStructure {
    fn default() -> Self {
        Self::new()
    }
}

impl FeeStructure {
    pub fn new() -> Self {
        Self {
            maker_fee: -0.0001,
            taker_fee: 0.0005,
            liquidation_fee: 0.003,
            funding_interval: 28800,
        }
    }

    pub fn calculate_fee(&self, is_maker: bool, notional_value: f64) -> f64 {
        let fee_rate = if is_maker {
            self.maker_fee
        } else {
            self.taker_fee
        };
        notional_value * fee_rate
    }
}

#[derive(Debug)]
pub struct InsuranceFund {
    pub balance: u64,
    pub target_ratio: f64,
    pub contributions: u64,
    pub payouts: u64,
}

impl InsuranceFund {
    pub fn new(initial_balance: u64) -> Self {
        Self {
            balance: initial_balance,
            target_ratio: 0.001,
            contributions: 0,
            payouts: 0,
        }
    }

    pub fn add_contribution(&mut self, amount: u64) {
        self.balance += amount;
        self.contributions += amount;
    }

    pub fn process_payout(&mut self, amount: u64) -> bool {
        if self.balance >= amount {
            self.balance -= amount;
            self.payouts += amount;
            true
        } else {
            false
        }
    }
}
