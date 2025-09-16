use crate::error::{OrderBookError, Result};
use crate::funding::FundingRate;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PositionSide {
    Long,
    Short,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub trader_id: u64,
    pub side: PositionSide,
    pub size: Decimal,
    pub entry_price: Decimal,
    pub margin: Decimal,
    pub leverage: Decimal,
    pub unrealized_pnl: Decimal,
    pub liquidation_price: Decimal,
    pub bankruptcy_price: Decimal,
}

#[derive(Debug, Clone)]
pub struct OraclePrice {
    pub price: Decimal,
    pub timestamp: u64,
    pub confidence: Decimal,
    pub source: String,
    price_history: VecDeque<(u64, Decimal)>,
}

impl OraclePrice {
    pub fn new(price: Decimal) -> Self {
        Self {
            price,
            timestamp: 0,
            confidence: dec!(0.99),
            source: "Simulated".to_string(),
            price_history: VecDeque::new(),
        }
    }

    pub fn update(&mut self, spot_price: Decimal) -> Result<()> {
        let noise = (rand::random::<f64>() - 0.5) * 0.001;
        let noise_decimal = Decimal::try_from(noise)
            .map_err(|e| OrderBookError::OverflowError(format!("Decimal conversion: {e}")))?;

        self.price = spot_price * (Decimal::ONE + noise_decimal);
        self.timestamp = self
            .timestamp
            .checked_add(1)
            .ok_or_else(|| OrderBookError::OverflowError("Timestamp overflow".to_string()))?;

        self.price_history.push_back((self.timestamp, self.price));
        if self.price_history.len() > 1000 {
            self.price_history.pop_front();
        }

        Ok(())
    }

    pub fn get_twap(&self, lookback_periods: usize) -> Decimal {
        if self.price_history.is_empty() {
            return self.price;
        }

        let samples: Vec<Decimal> = self
            .price_history
            .iter()
            .rev()
            .take(lookback_periods)
            .map(|(_, p)| *p)
            .collect();

        if samples.is_empty() {
            return self.price;
        }

        samples.iter().sum::<Decimal>() / Decimal::from(samples.len())
    }
}

#[derive(Debug, Clone)]
pub struct MarkPrice {
    pub price: Decimal,
    pub fair_price: Decimal,
    pub index_price: Decimal,
    pub funding_basis: Decimal,
    price_samples: VecDeque<(u64, Decimal, Decimal)>,
}

impl Default for MarkPrice {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkPrice {
    pub fn new() -> Self {
        Self {
            price: dec!(1000),
            fair_price: dec!(1000),
            index_price: dec!(1000),
            funding_basis: Decimal::ZERO,
            price_samples: VecDeque::new(),
        }
    }

    pub fn calculate(
        &mut self,
        best_bid: Decimal,
        best_ask: Decimal,
        index_price: Decimal,
    ) -> Result<()> {
        if best_bid <= Decimal::ZERO || best_ask <= Decimal::ZERO {
            return Err(OrderBookError::InvalidPrice(
                "Invalid bid/ask prices".to_string(),
            ));
        }

        if best_bid > best_ask {
            return Err(OrderBookError::MarketManipulation(
                "Crossed market detected".to_string(),
            ));
        }

        self.fair_price = (best_bid + best_ask) / dec!(2);
        self.index_price = index_price;

        let basis = self.fair_price - self.index_price;
        self.funding_basis = self.funding_basis * dec!(0.9) + basis * dec!(0.1);

        let impact_bid = best_bid * dec!(0.999);
        let impact_ask = best_ask * dec!(1.001);
        let impact_mid = (impact_bid + impact_ask) / dec!(2);

        self.price = (impact_mid + index_price * dec!(2)) / dec!(3);

        let timestamp = self.price_samples.len() as u64;
        self.price_samples
            .push_back((timestamp, self.price, index_price));
        if self.price_samples.len() > 100 {
            self.price_samples.pop_front();
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct LiquidationEngine {
    pub maintenance_margin: Decimal,
    pub initial_margin: Decimal,
    pub liquidation_fee: Decimal,
    pub insurance_fund: Decimal,
    pub adl_threshold: Decimal,
}

impl Default for LiquidationEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LiquidationEngine {
    pub fn new() -> Self {
        Self {
            maintenance_margin: dec!(0.005),
            initial_margin: dec!(0.01),
            liquidation_fee: dec!(0.003),
            insurance_fund: dec!(1000000),
            adl_threshold: dec!(0.8),
        }
    }

    pub fn calculate_liquidation_price(&self, position: &Position) -> Result<Decimal> {
        if position.leverage <= Decimal::ZERO {
            return Err(OrderBookError::InvalidLeverage(
                position.leverage.to_f64().unwrap_or(0.0),
            ));
        }

        let margin_ratio = self.maintenance_margin + self.liquidation_fee;

        let liq_price = match position.side {
            PositionSide::Long => {
                position.entry_price * (Decimal::ONE - margin_ratio / position.leverage)
            }
            PositionSide::Short => {
                position.entry_price * (Decimal::ONE + margin_ratio / position.leverage)
            }
        };

        Ok(liq_price.max(Decimal::ZERO))
    }

    pub fn calculate_bankruptcy_price(&self, position: &Position) -> Result<Decimal> {
        if position.size == Decimal::ZERO {
            return Err(OrderBookError::InvalidQuantity(
                "Position size is zero".to_string(),
            ));
        }

        let bankruptcy_price = match position.side {
            PositionSide::Long => position.entry_price - (position.margin / position.size),
            PositionSide::Short => position.entry_price + (position.margin / position.size),
        };

        Ok(bankruptcy_price.max(Decimal::ZERO))
    }

    pub fn should_liquidate(&self, position: &Position, mark_price: Decimal) -> bool {
        match position.side {
            PositionSide::Long => mark_price <= position.liquidation_price,
            PositionSide::Short => mark_price >= position.liquidation_price,
        }
    }

    pub fn calculate_pnl(position: &Position, mark_price: Decimal) -> Decimal {
        let price_diff = mark_price - position.entry_price;
        match position.side {
            PositionSide::Long => price_diff * position.size,
            PositionSide::Short => -price_diff * position.size,
        }
    }

    pub fn calculate_margin_ratio(
        &self,
        position: &Position,
        mark_price: Decimal,
    ) -> Result<Decimal> {
        let position_value = mark_price * position.size;
        if position_value == Decimal::ZERO {
            return Err(OrderBookError::InvalidQuantity(
                "Position value is zero".to_string(),
            ));
        }

        let pnl = Self::calculate_pnl(position, mark_price);
        Ok((position.margin + pnl) / position_value)
    }

    pub fn should_trigger_adl(&self) -> bool {
        let total_positions_value = dec!(10000000);
        self.insurance_fund / total_positions_value < self.adl_threshold
    }
}

#[derive(Debug)]
pub struct PositionManager {
    pub positions: HashMap<u64, Position>,
    pub total_long_interest: Decimal,
    pub total_short_interest: Decimal,
    pub max_leverage: Decimal,
    pub max_position_size: Decimal,
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
            total_long_interest: Decimal::ZERO,
            total_short_interest: Decimal::ZERO,
            max_leverage: dec!(100),
            max_position_size: dec!(1000000),
        }
    }

    pub fn open_position(
        &mut self,
        trader_id: u64,
        side: PositionSide,
        size: Decimal,
        entry_price: Decimal,
        margin: Decimal,
        liquidation_engine: &LiquidationEngine,
    ) -> Result<Position> {
        if size > self.max_position_size {
            return Err(OrderBookError::InvalidQuantity(format!(
                "Position size {} exceeds maximum {}",
                size, self.max_position_size
            )));
        }

        if margin <= Decimal::ZERO {
            return Err(OrderBookError::InsufficientMargin {
                required: 1,
                provided: 0,
            });
        }

        let leverage = (entry_price * size) / margin;
        if leverage > self.max_leverage {
            return Err(OrderBookError::InvalidLeverage(
                leverage.to_f64().unwrap_or(0.0),
            ));
        }

        let required_margin = (entry_price * size * liquidation_engine.initial_margin).round_dp(2);

        if margin < required_margin {
            return Err(OrderBookError::InsufficientMargin {
                required: required_margin.to_u64().unwrap_or(0),
                provided: margin.to_u64().unwrap_or(0),
            });
        }

        let mut position = Position {
            trader_id,
            side,
            size,
            entry_price,
            margin,
            leverage,
            unrealized_pnl: Decimal::ZERO,
            liquidation_price: Decimal::ZERO,
            bankruptcy_price: Decimal::ZERO,
        };

        position.liquidation_price = liquidation_engine.calculate_liquidation_price(&position)?;
        position.bankruptcy_price = liquidation_engine.calculate_bankruptcy_price(&position)?;

        match side {
            PositionSide::Long => self.total_long_interest += size,
            PositionSide::Short => self.total_short_interest += size,
        }

        self.positions.insert(trader_id, position.clone());
        Ok(position)
    }

    pub fn close_position(&mut self, trader_id: u64) -> Result<Position> {
        let position = self
            .positions
            .remove(&trader_id)
            .ok_or(OrderBookError::PositionNotFound { trader_id })?;

        match position.side {
            PositionSide::Long => {
                self.total_long_interest = self
                    .total_long_interest
                    .checked_sub(position.size)
                    .ok_or_else(|| {
                        OrderBookError::OverflowError("Long interest underflow".to_string())
                    })?;
            }
            PositionSide::Short => {
                self.total_short_interest = self
                    .total_short_interest
                    .checked_sub(position.size)
                    .ok_or_else(|| {
                        OrderBookError::OverflowError("Short interest underflow".to_string())
                    })?;
            }
        }

        Ok(position)
    }

    pub fn update_positions(
        &mut self,
        mark_price: Decimal,
        liquidation_engine: &LiquidationEngine,
    ) -> Result<Vec<u64>> {
        let mut liquidated = Vec::new();

        for (trader_id, position) in self.positions.iter_mut() {
            position.unrealized_pnl = LiquidationEngine::calculate_pnl(position, mark_price);

            if liquidation_engine.should_liquidate(position, mark_price) {
                liquidated.push(*trader_id);
            }
        }

        for trader_id in &liquidated {
            self.close_position(*trader_id)?;
        }

        Ok(liquidated)
    }

    pub fn apply_funding(&mut self, funding_rate: &FundingRate) -> HashMap<u64, Decimal> {
        let mut funding_payments = HashMap::new();

        for (trader_id, position) in self.positions.iter_mut() {
            let is_long = matches!(position.side, PositionSide::Long);
            let payment = funding_rate.calculate_funding_payment(position.size, is_long);

            position.margin += payment;
            funding_payments.insert(*trader_id, payment);
        }

        funding_payments
    }
}

#[derive(Debug, Clone)]
pub struct FeeStructure {
    pub maker_fee: Decimal,
    pub taker_fee: Decimal,
    pub liquidation_fee: Decimal,
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
            maker_fee: dec!(-0.0001),
            taker_fee: dec!(0.0005),
            liquidation_fee: dec!(0.003),
            funding_interval: 28800,
        }
    }

    pub fn calculate_fee(&self, is_maker: bool, notional_value: Decimal) -> Decimal {
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
    pub balance: Decimal,
    pub target_ratio: Decimal,
    pub contributions: Decimal,
    pub payouts: Decimal,
}

impl InsuranceFund {
    pub fn new(initial_balance: Decimal) -> Self {
        Self {
            balance: initial_balance,
            target_ratio: dec!(0.001),
            contributions: Decimal::ZERO,
            payouts: Decimal::ZERO,
        }
    }

    pub fn add_contribution(&mut self, amount: Decimal) -> Result<()> {
        self.balance = self
            .balance
            .checked_add(amount)
            .ok_or_else(|| OrderBookError::OverflowError("Insurance fund overflow".to_string()))?;
        self.contributions = self
            .contributions
            .checked_add(amount)
            .ok_or_else(|| OrderBookError::OverflowError("Contributions overflow".to_string()))?;
        Ok(())
    }

    pub fn process_payout(&mut self, amount: Decimal) -> Result<bool> {
        if self.balance >= amount {
            self.balance = self.balance.checked_sub(amount).ok_or_else(|| {
                OrderBookError::OverflowError("Insurance fund underflow".to_string())
            })?;
            self.payouts = self
                .payouts
                .checked_add(amount)
                .ok_or_else(|| OrderBookError::OverflowError("Payouts overflow".to_string()))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
