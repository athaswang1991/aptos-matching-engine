# In-Memory Limit Order Book

High-performance order book implementation with two modes: traditional order book and perpetual futures DEX.

## Quick Start

```bash
# Run order book TUI
cargo run --bin imlob

# Run perpetual futures DEX demo
cargo run --bin perps_demo

# Run simple CLI demo
cargo run --bin demo

# Run benchmarks
cargo bench
```

## Mode 1: Traditional Order Book

### Features
- Price-time priority (FIFO) matching
- Partial fill support
- Sub-microsecond execution latency
- Real-time TUI with market visualization

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

### Visualization (TUI)
- Order book depth chart with volume bars
- Real-time price chart
- Trade execution feed
- Latency metrics (execution & data feed)
- Market scenarios (normal, volatile, flash crash, recovery)

### Controls
- `Space` - Pause/Resume
- `+/-` - Adjust speed
- `Q` - Quit

## Mode 2: Perpetual Futures DEX

### Core Components

#### Funding Rate
- Automatic calculation based on mark/index premium
- Periodic payments between longs and shorts
- Anchors perpetual price to spot

#### Mark Price
- Prevents manipulation and cascade liquidations
- Formula: `Index Price + EMA(Fair Price - Index Price)`
- Used for PnL and liquidations

#### Oracle Integration
- External price feeds simulation
- Index price with confidence intervals
- Decoupled from order book

#### Leverage Trading
- Up to 100x leverage
- Initial margin: 1%
- Maintenance margin: 0.5%
- Automatic liquidation calculation

#### Position Management
- Long/short tracking
- Real-time PnL calculation
- Margin ratio monitoring
- Health indicators

#### Fee Structure
- Maker rebate: -0.01%
- Taker fee: 0.05%
- Liquidation penalty: 0.3%

#### Insurance Fund
- Socialized loss protection
- Funded by liquidation fees
- Target ratio management

### Data Structures

```rust
Position {
    trader_id: u64,
    side: PositionSide,
    size: u64,
    entry_price: f64,
    margin: u64,
    leverage: f64,
    unrealized_pnl: f64,
    liquidation_price: f64,
}

FundingRate {
    rate: f64,
    premium_index: f64,
    long_open_interest: u64,
    short_open_interest: u64,
}

MarkPrice {
    price: f64,
    fair_price: f64,
    index_price: f64,
    funding_basis: f64,
}
```

## Architecture

### Order Book
- `BTreeMap` for O(log n) price level operations
- `VecDeque` for O(1) FIFO at each level
- Custom `BuyPrice` wrapper for reverse ordering
- Batch removal of exhausted levels

### Optimizations
- Inline hints for hot paths
- Zero-copy where possible
- Efficient matching algorithm
- Pre-allocated buffers

## Testing

```bash
cargo test
```

Comprehensive test coverage including:
- Order matching logic
- Price-time priority
- Partial fills
- Liquidations
- Funding rate calculations

## Use Cases

- High-frequency trading systems
- Market making algorithms
- Exchange matching engines
- Perpetual futures DEX
- Trading simulations
- Financial education

## Dependencies

- `ratatui` - Terminal UI
- `crossterm` - Terminal control
- `rand` - Market simulation

## License

MIT