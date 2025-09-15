# CLOB-Based Perpetual Futures DEX

High-performance Central Limit Order Book (CLOB) implementation in Rust, designed for perpetual futures DEX infrastructure.

## Architecture

This is a **CLOB-based perpetual futures DEX** where all trading occurs through a central limit order book with transparent price discovery and fair order matching. Unlike AMM-based DEXs, this provides:

- Full order book depth visibility
- Transparent price discovery through limit orders
- No slippage beyond the order book spread
- Fair price-time priority matching
- No MEV from sandwich attacks

## Quick Start

```bash
# Run CLOB TUI visualization
cargo run --bin imlob

# Run perpetual futures DEX demo
cargo run --bin perps_demo

# Run simple CLOB demo
cargo run --bin demo

# Run benchmarks
cargo bench
```

## Core CLOB Engine

### Features
- **Central Limit Order Book** with full depth transparency
- Price-time priority (FIFO) matching algorithm
- Partial fill support for large orders
- Sub-microsecond execution latency
- No off-chain order flow or dark pools

### API
```rust
place_order(side: Side, price: u64, quantity: u64, id: u64) -> Vec<Trade>
best_buy() -> Option<(price, total_quantity)>
best_sell() -> Option<(price, total_quantity)>
```

### Performance
- **21M+ orders/second** throughput
- **39M+ trades/second** matching
- **Sub-microsecond** execution latency

### CLOB Visualization (TUI)
- Real-time order book depth with buy/sell walls
- Price-level aggregation with volume bars
- Live price chart tracking mid-market
- Trade execution feed
- Latency metrics (execution & data feed)
- Market scenarios simulation

### Controls
- `Space` - Pause/Resume
- `+/-` - Adjust speed
- `Q` - Quit

## Perpetual Futures DEX Layer

Built on top of the CLOB engine, the perpetual futures layer adds derivatives trading capabilities while maintaining the transparent, fair matching of the underlying order book.

### Key Components

#### CLOB Integration
- All orders go through the central limit order book
- No synthetic liquidity or virtual AMM
- Real order depth from actual limit orders
- Transparent price discovery

#### Funding Rate Mechanism
- Calculated from order book premium vs oracle price
- Periodic payments between longs and shorts
- Anchors perpetual price to spot market
- Based on actual CLOB trading activity

#### Mark Price
- Derived from CLOB fair price and oracle
- Prevents manipulation and cascade liquidations
- Formula: `Index Price + EMA(CLOB_Fair_Price - Index Price)`
- Used for PnL and liquidations

#### Oracle Integration
- External spot price feeds
- Index price with confidence intervals
- Provides reference for funding rates
- Independent from CLOB trading

#### Leverage Trading
- Up to 100x leverage
- Initial margin: 1%
- Maintenance margin: 0.5%
- Automatic liquidation when margin depleted
- Liquidations executed through CLOB

#### Position Management
- Long/short position tracking
- Real-time PnL calculation
- Margin ratio monitoring
- Health indicators
- All trades matched through CLOB

#### Fee Structure
- Maker rebate: -0.01% (incentivizes liquidity)
- Taker fee: 0.05%
- Liquidation penalty: 0.3%
- Fees encourage CLOB liquidity provision

#### Insurance Fund
- Backstop for underwater positions
- Funded by liquidation fees
- Protects against socialized losses
- Maintains CLOB integrity

### Data Structures

```rust
Position {
    trader_id: u64,
    side: PositionSide,
    size: u64,
    entry_price: f64,      // CLOB execution price
    margin: u64,
    leverage: f64,
    unrealized_pnl: f64,   // Based on mark price
    liquidation_price: f64,
}

FundingRate {
    rate: f64,
    premium_index: f64,    // CLOB price vs Oracle
    long_open_interest: u64,
    short_open_interest: u64,
}

MarkPrice {
    price: f64,
    fair_price: f64,       // From CLOB mid-market
    index_price: f64,      // From oracle
    funding_basis: f64,    // EMA of premium
}
```

## Technical Implementation

### CLOB Data Structure
- `BTreeMap` for O(log n) price level operations
- `VecDeque` for O(1) FIFO order matching at each level
- Custom `BuyPrice` wrapper for bid-side ordering
- Efficient order cancellation and modification

### Optimizations
- Inline hints for hot paths
- Zero-copy matching engine
- Lock-free price updates
- Pre-allocated order pools

## Testing

```bash
cargo test
```

Comprehensive test coverage:
- CLOB matching logic
- Price-time priority enforcement
- Partial fill scenarios
- Cross-spread matching
- Liquidation triggers
- Funding rate calculations

## Use Cases

- **Decentralized Perpetual Futures Exchange**
- **On-chain CLOB DEX**
- **High-frequency trading systems**
- **Market making on DEX**
- **Transparent price discovery**
- **Fair order matching without MEV**
- **Trading simulations and backtesting**

## Why CLOB for Perps DEX?

Unlike AMM-based perpetual DEXs, a CLOB-based approach provides:

1. **True Price Discovery** - Prices set by limit orders, not formulas
2. **No Impermanent Loss** - No LP positions, just order matching
3. **Professional Trading** - Limit orders, stop losses, iceberg orders
4. **Capital Efficiency** - No locked liquidity in pools
5. **Transparent Execution** - See exact depth and your queue position
6. **Fair Matching** - Price-time priority, no front-running

## Dependencies

- `ratatui` - Terminal UI for CLOB visualization
- `crossterm` - Cross-platform terminal control
- `rand` - Market simulation

## License

MIT