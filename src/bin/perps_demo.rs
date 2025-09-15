use imlob::perps::*;
use imlob::{OrderBook, Side};
use rand::Rng;
use std::{thread, time::Duration};

fn main() {
    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║     🚀 PERPETUAL FUTURES DEX - ORDER BOOK DEMO 🚀        ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");

    let mut order_book = OrderBook::new();
    let mut position_manager = PositionManager::new();
    let mut funding_rate = FundingRate::new();
    let mut mark_price = MarkPrice::new();
    let mut oracle = OraclePrice::new(1000.0);
    let liquidation_engine = LiquidationEngine::new();
    let fee_structure = FeeStructure::new();
    let mut insurance_fund = InsuranceFund::new(1000000);

    let mut rng = rand::thread_rng();
    let mut order_id = 1u64;
    let mut trader_id = 1u64;

    println!("📊 Initial Market Setup");
    println!("═══════════════════════════════════════════════════════");
    println!("  Max Leverage:        {}x", position_manager.max_leverage);
    println!(
        "  Initial Margin:      {}%",
        liquidation_engine.initial_margin * 100.0
    );
    println!(
        "  Maintenance Margin:  {}%",
        liquidation_engine.maintenance_margin * 100.0
    );
    println!(
        "  Maker Fee:           {}%",
        fee_structure.maker_fee * 100.0
    );
    println!(
        "  Taker Fee:           {}%",
        fee_structure.taker_fee * 100.0
    );
    println!("  Insurance Fund:      ${}\n", insurance_fund.balance);

    println!("🌊 Seeding Order Book with Initial Liquidity...\n");
    for i in 0..10 {
        let buy_price = 995 - i;
        let sell_price = 1005 + i;
        order_book.place_order(Side::Buy, buy_price, 1000, order_id);
        order_id += 1;
        order_book.place_order(Side::Sell, sell_price, 1000, order_id);
        order_id += 1;
    }

    for round in 1..=15 {
        println!("\n╔═══════════════════════════════════════════════════════════╗");
        println!(
            "║                      ROUND {round}                            ║"
        );
        println!("╚═══════════════════════════════════════════════════════════╝");

        let spot_movement = rng.gen_range(-5.0..5.0);
        oracle.price += spot_movement;
        oracle.update(oracle.price);

        let (best_bid, best_ask) = match (order_book.best_buy(), order_book.best_sell()) {
            (Some((bid, _)), Some((ask, _))) => (bid as f64, ask as f64),
            _ => (oracle.price - 1.0, oracle.price + 1.0),
        };
        mark_price.calculate(best_bid, best_ask, oracle.price);

        funding_rate.calculate_rate(mark_price.price, oracle.price);
        funding_rate.update_open_interest(
            position_manager.total_long_interest,
            position_manager.total_short_interest,
        );

        println!("\n📈 Market Prices:");
        println!("  Oracle/Index Price:  ${:.2}", oracle.price);
        println!("  Mark Price:          ${:.2}", mark_price.price);
        println!("  Fair Price:          ${:.2}", mark_price.fair_price);
        println!("  Best Bid/Ask:        ${best_bid:.2} / ${best_ask:.2}");
        println!("  Spread:              ${:.2}", best_ask - best_bid);

        println!("\n💰 Funding Rate:");
        println!(
            "  Current Rate:        {:.4}% per 8h",
            funding_rate.rate * 100.0
        );
        println!(
            "  Premium Index:       {:.4}%",
            funding_rate.premium_index * 100.0
        );
        if funding_rate.rate > 0.0 {
            println!("  Direction:           Longs pay Shorts ↗️");
        } else if funding_rate.rate < 0.0 {
            println!("  Direction:           Shorts pay Longs ↘️");
        } else {
            println!("  Direction:           Neutral ➡️");
        }

        let action = rng.gen_range(0..100);

        if action < 30 && position_manager.positions.len() < 10 {
            let side = if rng.gen_bool(0.5) {
                PositionSide::Long
            } else {
                PositionSide::Short
            };
            let size = rng.gen_range(100..1000);
            let leverage = rng.gen_range(1.0..50.0);
            let margin = ((mark_price.price * size as f64) / leverage) as u64;

            let position = position_manager.open_position(
                trader_id,
                side,
                size,
                mark_price.price,
                margin,
                &liquidation_engine,
            );

            println!("\n🆕 New Position Opened:");
            println!("  Trader #{trader_id}:");
            println!("  Side:                {side:?}");
            println!("  Size:                {size} contracts");
            println!("  Leverage:            {leverage:.1}x");
            println!("  Entry Price:         ${:.2}", position.entry_price);
            println!("  Margin:              ${margin}");
            println!("  Liquidation Price:   ${:.2}", position.liquidation_price);

            let notional = mark_price.price * size as f64;
            let fee = fee_structure.calculate_fee(false, notional);
            println!("  Fee Paid:            ${:.2}", fee.abs());

            trader_id += 1;
        }

        let liquidated = position_manager.update_positions(mark_price.price, &liquidation_engine);
        if !liquidated.is_empty() {
            println!("\n⚠️  LIQUIDATIONS:");
            for trader in liquidated {
                println!(
                    "  🔴 Trader #{} position liquidated at ${:.2}",
                    trader, mark_price.price
                );

                let liquidation_fee_amount = 1000;
                insurance_fund.add_contribution(liquidation_fee_amount);
            }
        }

        println!("\n📊 Open Interest:");
        println!(
            "  Total Long:          {} contracts",
            position_manager.total_long_interest
        );
        println!(
            "  Total Short:         {} contracts",
            position_manager.total_short_interest
        );
        let imbalance = position_manager.total_long_interest as f64
            - position_manager.total_short_interest as f64;
        let total_oi = position_manager.total_long_interest + position_manager.total_short_interest;
        if total_oi > 0 {
            let imbalance_pct = (imbalance / total_oi as f64) * 100.0;
            println!(
                "  Imbalance:           {:.1}% {}",
                imbalance_pct.abs(),
                if imbalance > 0.0 {
                    "(Long heavy)"
                } else if imbalance < 0.0 {
                    "(Short heavy)"
                } else {
                    "(Balanced)"
                }
            );
        }

        if !position_manager.positions.is_empty() {
            println!("\n💼 Active Positions (Top 3):");
            let mut positions: Vec<_> = position_manager.positions.values().collect();
            positions.sort_by(|a, b| b.size.cmp(&a.size));

            for (i, pos) in positions.iter().take(3).enumerate() {
                let pnl = LiquidationEngine::calculate_pnl(pos, mark_price.price);
                let margin_ratio = liquidation_engine.calculate_margin_ratio(pos, mark_price.price);
                let health = if margin_ratio > 0.02 {
                    "🟢"
                } else if margin_ratio > 0.01 {
                    "🟡"
                } else {
                    "🔴"
                };

                println!(
                    "  {}. Trader #{}: {:?} {} @ ${:.2} | PnL: ${:.2} | Margin: {:.2}% {}",
                    i + 1,
                    pos.trader_id,
                    pos.side,
                    pos.size,
                    pos.entry_price,
                    pnl,
                    margin_ratio * 100.0,
                    health
                );
            }
        }

        for _ in 0..5 {
            let side = if rng.gen_bool(0.5) {
                Side::Buy
            } else {
                Side::Sell
            };
            let price = if side == Side::Buy {
                (mark_price.price - rng.gen_range(1.0..10.0)) as u64
            } else {
                (mark_price.price + rng.gen_range(1.0..10.0)) as u64
            };
            let qty = rng.gen_range(100..1000);
            order_book.place_order(side, price, qty, order_id);
            order_id += 1;
        }

        println!(
            "\n🛡️  Insurance Fund: ${} (Contributions: ${}, Payouts: ${})",
            insurance_fund.balance, insurance_fund.contributions, insurance_fund.payouts
        );

        thread::sleep(Duration::from_secs(2));
    }

    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║                    SIMULATION COMPLETE                     ║");
    println!("╚═══════════════════════════════════════════════════════════╝");

    println!("\n📊 Final Statistics:");
    println!("  Total Positions Opened:     {}", trader_id - 1);
    println!(
        "  Active Positions:           {}",
        position_manager.positions.len()
    );
    println!("  Final Mark Price:           ${:.2}", mark_price.price);
    println!("  Final Index Price:          ${:.2}", oracle.price);
    println!("  Insurance Fund Balance:     ${}", insurance_fund.balance);

    println!("\n✨ Key Perpetual DEX Features Demonstrated:");
    println!("  ✅ Funding Rate Mechanism (longs/shorts pay based on premium)");
    println!("  ✅ Mark Price Calculation (prevents manipulation)");
    println!("  ✅ Oracle Price Integration (external price feed)");
    println!("  ✅ Leverage Trading (up to 100x)");
    println!("  ✅ Liquidation Engine (with insurance fund)");
    println!("  ✅ Position Management & PnL Tracking");
    println!("  ✅ Maker/Taker Fee Structure");
    println!("  ✅ Open Interest Tracking");
}
