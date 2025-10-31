use aptos_matching_engine::{OrderBook, Side};
use rust_decimal::Decimal;
use std::time::Instant;

fn benchmark_place_orders(n: usize) {
    let mut book = OrderBook::new();
    let start = Instant::now();

    // Place alternating buy and sell orders
    for i in 0..n {
        let side = if i % 2 == 0 { Side::Buy } else { Side::Sell };
        let price = Decimal::from(1000 + (i % 100));
        let _ = book.place_order(side, price, Decimal::from(100), i as u64);
    }

    let elapsed = start.elapsed();
    println!(
        "Placed {} orders in {:.2}ms ({:.0} orders/sec)",
        n,
        elapsed.as_secs_f64() * 1000.0,
        n as f64 / elapsed.as_secs_f64()
    );
}

fn benchmark_matching(n: usize) {
    let mut book = OrderBook::new();

    // Fill book with buy orders
    for i in 0..n / 2 {
        let _ = book.place_order(Side::Buy, Decimal::from(100), Decimal::from(10), i as u64);
    }

    let start = Instant::now();

    // Match with sell orders
    for i in 0..n / 2 {
        let _ = book.place_order(
            Side::Sell,
            Decimal::from(100),
            Decimal::from(10),
            (n / 2 + i) as u64,
        );
    }

    let elapsed = start.elapsed();
    println!(
        "Matched {} orders in {:.2}ms ({:.0} matches/sec)",
        n / 2,
        elapsed.as_secs_f64() * 1000.0,
        (n / 2) as f64 / elapsed.as_secs_f64()
    );
}

fn benchmark_best_price_queries(n: usize) {
    let mut book = OrderBook::new();

    // Fill book with orders at different prices
    for i in 0..100 {
        let _ = book.place_order(Side::Buy, Decimal::from(900 + i), Decimal::from(100), i);
        let _ = book.place_order(
            Side::Sell,
            Decimal::from(1100 + i),
            Decimal::from(100),
            100 + i,
        );
    }

    let start = Instant::now();

    for _ in 0..n {
        let _ = book.best_buy();
        let _ = book.best_sell();
    }

    let elapsed = start.elapsed();
    println!(
        "Performed {} best price queries in {:.2}ms ({:.0} queries/sec)",
        n * 2,
        elapsed.as_secs_f64() * 1000.0,
        (n * 2) as f64 / elapsed.as_secs_f64()
    );
}

fn benchmark_cross_spread_matching() {
    let mut book = OrderBook::new();

    // Create deep book
    for i in 0..1000 {
        let _ = book.place_order(
            Side::Buy,
            Decimal::from(990 - i / 10),
            Decimal::from(100),
            i,
        );
        let _ = book.place_order(
            Side::Sell,
            Decimal::from(1010 + i / 10),
            Decimal::from(100),
            1000 + i,
        );
    }

    let start = Instant::now();

    // Large aggressive order that crosses the spread
    let trades = book.place_order(Side::Sell, Decimal::from(900), Decimal::from(50000), 10000);

    let elapsed = start.elapsed();
    println!(
        "Cross-spread matching: {} trades in {:.2}ms",
        trades.unwrap().len(),
        elapsed.as_secs_f64() * 1000.0
    );
}

fn main() {
    println!("=== OrderBook Performance Benchmarks ===\n");

    println!("Small workload:");
    benchmark_place_orders(1_000);
    benchmark_matching(1_000);
    benchmark_best_price_queries(10_000);

    println!("\nMedium workload:");
    benchmark_place_orders(10_000);
    benchmark_matching(10_000);
    benchmark_best_price_queries(100_000);

    println!("\nLarge workload:");
    benchmark_place_orders(100_000);
    benchmark_matching(100_000);
    benchmark_best_price_queries(1_000_000);

    println!("\nComplex matching:");
    benchmark_cross_spread_matching();
}
