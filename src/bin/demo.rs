use imlob::{OrderBook, Side};
use rand::Rng;
use std::{thread, time::Duration};

fn main() {
    println!("\n═══════════════════════════════════════════════════════");
    println!("        📈 LIMIT ORDER BOOK DEMO");
    println!("═══════════════════════════════════════════════════════\n");

    let mut book = OrderBook::new();
    let mut rng = rand::thread_rng();
    let mut order_id = 1u64;
    let mut total_trades = 0;
    let mut total_volume = 0u64;

    println!("🔧 Initial Setup - Adding liquidity to the book...\n");

    for i in 0..5 {
        let buy_price = 995 - i;
        let sell_price = 1005 + i;

        book.place_order(Side::Buy, buy_price, 100, order_id);
        println!(
            "  → place_order(Buy, {buy_price}, 100, #{order_id}) = []"
        );
        order_id += 1;

        book.place_order(Side::Sell, sell_price, 100, order_id);
        println!(
            "  → place_order(Sell, {sell_price}, 100, #{order_id}) = []"
        );
        order_id += 1;
    }

    display_book_state(&book);
    thread::sleep(Duration::from_secs(2));

    println!("\n🚀 Starting continuous order flow simulation...\n");
    println!("───────────────────────────────────────────────────────");

    for round in 1..=10 {
        println!("\n📊 Round {round}/10");
        println!("───────────────────────────────────────────────────────");

        let side = if rng.gen_bool(0.5) {
            Side::Buy
        } else {
            Side::Sell
        };
        let is_aggressive = rng.gen_bool(0.4);

        let (price, quantity) = if is_aggressive {
            match side {
                Side::Buy => (1000 + rng.gen_range(5..15), rng.gen_range(50..300)),
                Side::Sell => (1000 - rng.gen_range(5..15), rng.gen_range(50..300)),
            }
        } else {
            match side {
                Side::Buy => (995 - rng.gen_range(0..5), rng.gen_range(50..150)),
                Side::Sell => (1005 + rng.gen_range(0..5), rng.gen_range(50..150)),
            }
        };

        println!("\n🔹 API CALL:");
        println!(
            "  place_order({side:?}, {price}, {quantity}, #{order_id})"
        );

        let trades = book.place_order(side, price, quantity, order_id);

        if trades.is_empty() {
            println!("\n🔸 RETURN: Vec::new() (no matches)");
            println!("  → Order #{order_id} added to book");
        } else {
            println!("\n🔸 RETURN: {} trade(s) executed:", trades.len());
            for trade in &trades {
                println!(
                    "  → Trade {{ price: {}, qty: {}, maker: #{}, taker: #{} }}",
                    trade.price, trade.quantity, trade.maker_id, trade.taker_id
                );
                total_trades += 1;
                total_volume += trade.quantity;
            }
        }

        order_id += 1;

        display_book_state(&book);

        thread::sleep(Duration::from_millis(1500));
    }

    println!("\n═══════════════════════════════════════════════════════");
    println!("                    📈 FINAL STATISTICS");
    println!("═══════════════════════════════════════════════════════");
    println!("  Total Trades Executed: {total_trades}");
    println!("  Total Volume Traded:   {total_volume}");
    println!("  Orders Placed:         {}", order_id - 1);

    display_book_state(&book);

    println!("\n✨ Demo completed successfully!");
}

fn display_book_state(book: &OrderBook) {
    println!("\n┌─────────────────────────────────────┐");
    println!("│         CURRENT BOOK STATE          │");
    println!("├─────────────────────────────────────┤");

    match book.best_sell() {
        Some((price, qty)) => {
            println!("│ best_sell() → Some({price}, {qty:<5})     │");
            println!("│         🔴 ASK: {qty} @ {price}          │");
        }
        None => {
            println!("│ best_sell() → None                  │");
            println!("│         🔴 ASK: (empty)             │");
        }
    }

    println!("│         ─ ─ ─ SPREAD ─ ─ ─         │");

    match book.best_buy() {
        Some((price, qty)) => {
            println!("│         🟢 BID: {qty} @ {price}          │");
            println!("│ best_buy() → Some({price}, {qty:<5})      │");
        }
        None => {
            println!("│         🟢 BID: (empty)             │");
            println!("│ best_buy() → None                   │");
        }
    }

    println!("└─────────────────────────────────────┘");

    if let (Some((bid_price, _)), Some((ask_price, _))) = (book.best_buy(), book.best_sell()) {
        let spread = ask_price as i64 - bid_price as i64;
        println!(
            "  Spread: {} | Book Depth: Buy={} Sell={}",
            spread,
            book.buy_depth(),
            book.sell_depth()
        );
    }
}
