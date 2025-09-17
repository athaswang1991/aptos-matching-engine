#[derive(Debug, Clone, Copy)]
pub enum MarketScenario {
    Normal,
    HighVolatility,
    FlashCrash,
    Recovery,
    LiquidityCrisis,
}
