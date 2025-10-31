use crate::simulator::scenarios::MarketScenario;
use crate::tui::app::App;
use aptos_matching_engine::types::Side;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, List, ListItem, Paragraph},
    Frame,
};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::time::Duration;

const BOOK_DEPTH: usize = 10;

fn scenario_color(scenario: MarketScenario) -> Color {
    match scenario {
        MarketScenario::Normal => Color::Green,
        MarketScenario::HighVolatility => Color::Yellow,
        MarketScenario::FlashCrash => Color::Red,
        MarketScenario::Recovery => Color::Cyan,
        MarketScenario::LiquidityCrisis => Color::Magenta,
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(20),
            Constraint::Length(6),
        ])
        .split(f.size());

    draw_header(f, chunks[0], app);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(chunks[1]);

    draw_order_book(f, main_chunks[0], app);
    draw_center_panel(f, main_chunks[1], app);
    draw_right_panel(f, main_chunks[2], app);

    draw_footer(f, chunks[2], app);
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let header_text = vec![
        Span::styled(
            "📈 High-Performance Order Book Demo | ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("Scenario: {:?} ", app.simulator.scenario),
            Style::default().fg(scenario_color(app.simulator.scenario)),
        ),
        Span::styled(
            format!("| Speed: {}ms ", app.update_interval.as_millis()),
            Style::default().fg(Color::Yellow),
        ),
        if app.paused {
            Span::styled(
                "| ⏸ PAUSED",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                "| ▶ RUNNING",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )
        },
    ];

    let header = Paragraph::new(Line::from(header_text))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .alignment(Alignment::Center);

    f.render_widget(header, area);
}

fn draw_order_book(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let sell_levels = app.get_book_levels(Side::Sell, BOOK_DEPTH);
    let buy_levels = app.get_book_levels(Side::Buy, BOOK_DEPTH);

    let max_qty = sell_levels
        .iter()
        .chain(buy_levels.iter())
        .map(|(_, qty)| *qty)
        .max()
        .unwrap_or(dec!(100))
        .max(dec!(100));

    let mid_price = match (app.order_book.best_buy(), app.order_book.best_sell()) {
        (Some((bid, _)), Some((ask, _))) => (bid + ask) / dec!(2),
        _ => app.simulator.mid_price(),
    };

    let sell_items: Vec<ListItem> = sell_levels
        .iter()
        .rev()
        .map(|(price, qty)| {
            let bar_width = 20;
            let bar_len = ((qty.to_f64().unwrap_or(0.0) * bar_width as f64)
                / max_qty.to_f64().unwrap_or(100.0)) as usize;
            let bar = "█".repeat(bar_len.min(bar_width));
            let padding = " ".repeat(bar_width - bar_len.min(bar_width));

            ListItem::new(format!("{price:>7.2} │ {qty:>8} │ {bar}{padding}"))
                .style(Style::default().fg(Color::Red))
        })
        .collect();

    let sell_list = List::new(sell_items)
        .block(Block::default().title("📉 Asks").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    let buy_items: Vec<ListItem> = buy_levels
        .iter()
        .map(|(price, qty)| {
            let bar_width = 20;
            let bar_len = ((qty.to_f64().unwrap_or(0.0) * bar_width as f64)
                / max_qty.to_f64().unwrap_or(100.0)) as usize;
            let bar = "█".repeat(bar_len.min(bar_width));
            let padding = " ".repeat(bar_width - bar_len.min(bar_width));

            ListItem::new(format!("{price:>7.2} │ {qty:>8} │ {bar}{padding}"))
                .style(Style::default().fg(Color::Green))
        })
        .collect();

    let buy_list = List::new(buy_items)
        .block(
            Block::default()
                .title(format!("📈 Bids | Mid: {mid_price:.2}"))
                .borders(Borders::ALL),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(sell_list, chunks[0]);
    f.render_widget(buy_list, chunks[1]);
}

fn draw_center_panel(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    draw_trades(f, chunks[0], app);
    draw_stats(f, chunks[1], app);
}

fn draw_trades(f: &mut Frame, area: Rect, app: &App) {
    let trades: Vec<ListItem> = app
        .trades
        .iter()
        .map(|(trade, timestamp)| {
            let age = timestamp.elapsed().as_secs();
            let age_str = if age < 1 {
                "now".to_string()
            } else {
                format!("{age}s ago")
            };

            let color = if trade.maker_id % 2 == 0 {
                Color::LightGreen
            } else {
                Color::LightRed
            };

            ListItem::new(format!(
                "{:>8} @ {:>7.2} │ M:{:>4} T:{:>4} │ {}",
                trade.quantity, trade.price, trade.maker_id, trade.taker_id, age_str
            ))
            .style(Style::default().fg(color))
        })
        .collect();

    let trades_list = List::new(trades).block(
        Block::default()
            .title(format!("💹 Recent Trades ({})", app.total_trades))
            .borders(Borders::ALL),
    );

    f.render_widget(trades_list, area);
}

fn draw_stats(f: &mut Frame, area: Rect, app: &App) {
    let stats_text = vec![
        Line::from(vec![
            Span::raw("📊 "),
            Span::styled(
                "Market Statistics",
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(format!(
            "Volume: {} | Avg Size: {:.2}",
            app.total_volume,
            if app.total_trades > 0 {
                app.total_volume / Decimal::from(app.total_trades)
            } else {
                Decimal::ZERO
            }
        )),
        Line::from(format!(
            "Spread: {:.2} | Imbalance: {:.1}%",
            app.market_stats.spread,
            app.market_stats.imbalance * 100.0
        )),
        Line::from(format!(
            "Bid Vol: {} | Ask Vol: {}",
            app.market_stats.bid_volume, app.market_stats.ask_volume
        )),
    ];

    let stats = Paragraph::new(stats_text)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Left);

    f.render_widget(stats, area);
}

fn draw_right_panel(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
        ])
        .split(area);

    draw_price_chart(f, chunks[0], app);
    draw_latency_metrics(f, chunks[1], app);
    draw_events(f, chunks[2], app);
}

fn draw_price_chart(f: &mut Frame, area: Rect, app: &App) {
    if app.price_history.is_empty() {
        let empty = Paragraph::new("Waiting for trades...")
            .block(
                Block::default()
                    .title("📈 Price Chart")
                    .borders(Borders::ALL),
            )
            .alignment(Alignment::Center);
        f.render_widget(empty, area);
        return;
    }

    let prices: Vec<(f64, f64)> = app
        .price_history
        .iter()
        .rev()
        .enumerate()
        .map(|(i, p)| (i as f64, p.to_f64().unwrap_or(0.0)))
        .collect();

    let min_price = app.price_history.iter().min().cloned().unwrap_or(dec!(0));
    let max_price = app
        .price_history
        .iter()
        .max()
        .cloned()
        .unwrap_or(dec!(1000));
    let price_range = max_price - min_price;
    let y_min = (min_price - price_range * dec!(0.1))
        .to_f64()
        .unwrap_or(0.0);
    let y_max = (max_price + price_range * dec!(0.1))
        .to_f64()
        .unwrap_or(1000.0);

    let datasets = vec![Dataset::default()
        .name("Price")
        .marker(symbols::Marker::Braille)
        .style(Style::default().fg(Color::Cyan))
        .graph_type(GraphType::Line)
        .data(&prices)];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .title("📈 Price Chart")
                .borders(Borders::ALL),
        )
        .x_axis(
            Axis::default()
                .bounds([0.0, prices.len() as f64])
                .labels(vec![]),
        )
        .y_axis(Axis::default().bounds([y_min, y_max]).labels(vec![
            format!("{y_min:.0}").into(),
            format!("{y_max:.0}").into(),
        ]));

    f.render_widget(chart, area);
}

fn draw_latency_metrics(f: &mut Frame, area: Rect, app: &App) {
    let metrics_text = vec![
        Line::from(vec![
            Span::raw("⚡ "),
            Span::styled("Performance", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(format!(
            "Execution: {:?} | Avg: {:?}",
            app.latency_metrics.last_execution.unwrap_or(Duration::ZERO),
            app.latency_metrics.avg_execution
        )),
        Line::from(format!(
            "P99 Exec: {:?} | P99 Feed: {:?}",
            app.latency_metrics.p99_execution, app.latency_metrics.p99_datafeed
        )),
    ];

    let metrics = Paragraph::new(metrics_text)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Left);

    f.render_widget(metrics, area);
}

fn draw_events(f: &mut Frame, area: Rect, app: &App) {
    let events: Vec<ListItem> = app
        .events
        .iter()
        .take(5)
        .map(|(msg, timestamp)| {
            let age = timestamp.elapsed().as_secs();
            let age_str = if age < 1 {
                "now".to_string()
            } else {
                format!("{age}s")
            };

            ListItem::new(format!("{age_str} │ {msg}")).style(Style::default().fg(Color::Gray))
        })
        .collect();

    let events_list =
        List::new(events).block(Block::default().title("📝 Events").borders(Borders::ALL));

    f.render_widget(events_list, area);
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let help_text = vec![
        Line::from(vec![
            Span::styled("Commands: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("Space = Pause/Resume │ "),
            Span::raw("+ = Speed Up │ "),
            Span::raw("- = Slow Down │ "),
            Span::raw("Q/Esc = Quit"),
        ]),
        Line::from(vec![
            Span::styled(
                "Book Stats: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("Buy Levels: {} │ ", app.order_book.buy_depth())),
            Span::raw(format!("Sell Levels: {} │ ", app.order_book.sell_depth())),
            Span::raw(format!(
                "Total Orders: {}",
                app.order_book.buy_depth() + app.order_book.sell_depth()
            )),
        ]),
    ];

    let footer = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);

    f.render_widget(footer, area);
}
