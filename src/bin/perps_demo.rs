use aptos_matching_engine::funding::FundingRate;
use aptos_matching_engine::perps::*;
use aptos_matching_engine::{OrderBook, Side};
use rand::Rng;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::{thread, time::Duration};

fn main() {
    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║     🚀 PERPETUAL FUTURES DEX - ORDER BOOK DEMO 🚀        ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");

    let mut order_book = OrderBook::new();
    let mut position_manager = PositionManager::new();
    let mut funding_rate = FundingRate::new();
    let mut mark_price = MarkPrice::new();
    let mut oracle = OraclePrice::new(dec!(1000));
    let liquidation_engine = LiquidationEngine::new();
    let fee_structure = FeeStructure::new();
    let mut insurance_fund = InsuranceFund::new(dec!(1000000));

    let mut rng = rand::thread_rng();
    let mut order_id = 1u64;
    let mut trader_id = 1u64;

    println!("📊 Initial Market Setup");
    println!("═══════════════════════════════════════════════════════");
    println!("  Max Leverage:        {}x", position_manager.max_leverage);
    println!(
        "  Initial Margin:      {}%",
        (liquidation_engine.initial_margin * dec!(100))
            .to_f64()
            .unwrap_or(0.0)
    );
    println!(
        "  Maintenance Margin:  {}%",
        (liquidation_engine.maintenance_margin * dec!(100))
            .to_f64()
            .unwrap_or(0.0)
    );
    println!(
        "  Maker Fee:           {}%",
        (fee_structure.maker_fee * dec!(100))
            .to_f64()
            .unwrap_or(0.0)
    );
    println!(
        "  Taker Fee:           {}%",
        (fee_structure.taker_fee * dec!(100))
            .to_f64()
            .unwrap_or(0.0)
    );
    println!("  Insurance Fund:      ${}\n", insurance_fund.balance);

    println!("🌊 Seeding Order Book with Initial Liquidity...\n");
    for i in 0..10 {
        let buy_price = dec!(995) - Decimal::from(i);
        let sell_price = dec!(1005) + Decimal::from(i);
        order_book
            .place_order(Side::Buy, buy_price, dec!(1000), order_id)
            .unwrap();
        order_id += 1;
        order_book
            .place_order(Side::Sell, sell_price, dec!(1000), order_id)
            .unwrap();
        order_id += 1;
    }

    for round in 1..=15 {
        println!("\n╔═══════════════════════════════════════════════════════════╗");
        println!("║                      ROUND {round}                            ║");
        println!("╚═══════════════════════════════════════════════════════════╝");

        let spot_movement = Decimal::try_from(rng.gen_range(-5.0..5.0)).unwrap_or(Decimal::ZERO);
        oracle.price += spot_movement;
        oracle.update(oracle.price).unwrap();

        let (best_bid, best_ask) = match (order_book.best_buy(), order_book.best_sell()) {
            (Some((bid, _)), Some((ask, _))) => (bid, ask),
            _ => (oracle.price - dec!(1), oracle.price + dec!(1)),
        };
        mark_price
            .calculate(best_bid, best_ask, oracle.price)
            .unwrap();

        funding_rate.add_price_sample(mark_price.price, oracle.price, round as u64 * 3600);
        let current_funding = funding_rate
            .calculate_funding_rate(round as u64 * 3600)
            .unwrap();
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
            (current_funding * dec!(100)).to_f64().unwrap_or(0.0)
        );
        println!(
            "  Premium Index:       {:.4}%",
            (funding_rate.premium_index * dec!(100))
                .to_f64()
                .unwrap_or(0.0)
        );
        if current_funding > Decimal::ZERO {
            println!("  Direction:           Longs pay Shorts ↗️");
        } else if current_funding < Decimal::ZERO {
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
            let size = Decimal::from(rng.gen_range(100..1000));
            let leverage = Decimal::try_from(rng.gen_range(1.0..50.0)).unwrap_or(dec!(10));
            let margin = ((mark_price.price * size) / leverage).round_dp(2);

            match position_manager.open_position(
                trader_id,
                side,
                size,
                mark_price.price,
                margin,
                &liquidation_engine,
            ) {
                Ok(position) => {
                    println!("\n🆕 New Position Opened:");
                    println!("  Trader #{trader_id}:");
                    println!("  Side:                {side:?}");
                    println!("  Size:                {size} contracts");
                    println!("  Leverage:            {leverage:.1}x");
                    println!("  Entry Price:         ${:.2}", position.entry_price);
                    println!("  Margin:              ${margin}");
                    println!("  Liquidation Price:   ${:.2}", position.liquidation_price);

                    let notional = mark_price.price * size;
                    let fee = fee_structure.calculate_fee(false, notional);
                    println!("  Fee Paid:            ${:.2}", fee.abs());

                    trader_id += 1;
                }
                Err(e) => {
                    println!("\n⚠️  Position opening failed: {e}");
                }
            }
        }

        match position_manager.update_positions(mark_price.price, &liquidation_engine) {
            Ok(liquidated) => {
                if !liquidated.is_empty() {
                    println!("\n⚠️  LIQUIDATIONS:");
                    for trader in liquidated {
                        println!(
                            "  🔴 Trader #{} position liquidated at ${:.2}",
                            trader, mark_price.price
                        );

                        let liquidation_fee_amount = dec!(1000);
                        insurance_fund
                            .add_contribution(liquidation_fee_amount)
                            .unwrap();
                    }
                }
            }
            Err(e) => {
                println!("\n⚠️  Error updating positions: {e}");
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
        let imbalance =
            position_manager.total_long_interest - position_manager.total_short_interest;
        let total_oi = position_manager.total_long_interest + position_manager.total_short_interest;
        if total_oi > Decimal::ZERO {
            let imbalance_pct = (imbalance / total_oi) * dec!(100);
            println!(
                "  Imbalance:           {:.1}% {}",
                imbalance_pct.abs(),
                if imbalance > Decimal::ZERO {
                    "(Long heavy)"
                } else if imbalance < Decimal::ZERO {
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
                let margin_ratio = liquidation_engine
                    .calculate_margin_ratio(pos, mark_price.price)
                    .unwrap_or(Decimal::ZERO);
                let health = if margin_ratio > dec!(0.02) {
                    "🟢"
                } else if margin_ratio > dec!(0.01) {
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
                    (margin_ratio * dec!(100)).to_f64().unwrap_or(0.0),
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
                mark_price.price - Decimal::from(rng.gen_range(1..10))
            } else {
                mark_price.price + Decimal::from(rng.gen_range(1..10))
            };
            let qty = Decimal::from(rng.gen_range(100..1000));
            order_book.place_order(side, price, qty, order_id).unwrap();
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
