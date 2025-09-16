#![allow(dead_code)]
#![allow(clippy::unnecessary_unwrap)]
#![allow(clippy::never_loop)]

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use imlob::{OrderBook, Side, Trade};
use rand::Rng;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, List, ListItem, Paragraph},
    Frame, Terminal,
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use rust_decimal::prelude::ToPrimitive;
use std::{
    collections::VecDeque,
    io,
    time::{Duration, Instant},
};

const MAX_TRADES: usize = 20;
const MAX_EVENTS: usize = 15;
const BOOK_DEPTH: usize = 10;
const LATENCY_HISTORY_SIZE: usize = 100;

#[derive(Debug, Clone, Copy)]
enum MarketScenario {
    Normal,
    HighVolatility,
    FlashCrash,
    Recovery,
    LiquidityCrisis,
}

struct MarketSimulator {
    next_order_id: u64,
    mid_price: Decimal,
    volatility: Decimal,
    scenario: MarketScenario,
    scenario_timer: u32,
}

impl MarketSimulator {
    fn new() -> Self {
        Self {
            next_order_id: 1,
            mid_price: dec!(1000),
            volatility: dec!(0.5),
            scenario: MarketScenario::Normal,
            scenario_timer: 0,
        }
    }

    fn update_scenario(&mut self) {
        let mut rng = rand::thread_rng();
        self.scenario_timer = self.scenario_timer.saturating_sub(1);

        if self.scenario_timer == 0 {
            self.scenario = match rng.gen_range(0..100) {
                0..=60 => MarketScenario::Normal,
                61..=75 => MarketScenario::HighVolatility,
                76..=85 => MarketScenario::FlashCrash,
                86..=95 => MarketScenario::Recovery,
                _ => MarketScenario::LiquidityCrisis,
            };
            self.scenario_timer = rng.gen_range(10..30);
        }
    }

    fn generate_order(&mut self) -> (Side, Decimal, Decimal, u64) {
        let mut rng = rand::thread_rng();
        self.update_scenario();

        let (volatility_mult, aggressive_prob, size_mult) = match self.scenario {
            MarketScenario::Normal => (dec!(1), 0.3, dec!(1)),
            MarketScenario::HighVolatility => (dec!(3), 0.5, dec!(1.5)),
            MarketScenario::FlashCrash => (dec!(10), 0.8, dec!(2)),
            MarketScenario::Recovery => (dec!(0.5), 0.2, dec!(0.8)),
            MarketScenario::LiquidityCrisis => (dec!(5), 0.1, dec!(0.3)),
        };

        let price_change = Decimal::from(rng.gen_range(-10..=10)) * self.volatility * volatility_mult / dec!(10);
        self.mid_price += price_change;
        self.mid_price = self.mid_price.max(dec!(900)).min(dec!(1100));

        let is_aggressive = rng.gen_bool(aggressive_prob);
        let side = if rng.gen_bool(0.5) {
            Side::Buy
        } else {
            Side::Sell
        };

        let price = if is_aggressive {
            match side {
                Side::Buy => self.mid_price + Decimal::from(rng.gen_range(5..15)),
                Side::Sell => self.mid_price - Decimal::from(rng.gen_range(5..15)),
            }
        } else {
            match side {
                Side::Buy => self.mid_price - Decimal::from(rng.gen_range(0..5)),
                Side::Sell => self.mid_price + Decimal::from(rng.gen_range(0..5)),
            }
        };

        let base_size = Decimal::from(rng.gen_range(50..200));
        let quantity = (base_size * size_mult).round();
        let id = self.next_order_id;
        self.next_order_id += 1;

        (side, price, quantity, id)
    }
}

struct LatencyMetrics {
    execution_latencies: VecDeque<Duration>,
    datafeed_latencies: VecDeque<Duration>,
    last_execution: Option<Duration>,
    last_datafeed: Option<Duration>,
    avg_execution: Duration,
    avg_datafeed: Duration,
    p99_execution: Duration,
    p99_datafeed: Duration,
}

impl LatencyMetrics {
    fn new() -> Self {
        Self {
            execution_latencies: VecDeque::new(),
            datafeed_latencies: VecDeque::new(),
            last_execution: None,
            last_datafeed: None,
            avg_execution: Duration::ZERO,
            avg_datafeed: Duration::ZERO,
            p99_execution: Duration::ZERO,
            p99_datafeed: Duration::ZERO,
        }
    }

    fn record_execution(&mut self, latency: Duration) {
        self.last_execution = Some(latency);
        self.execution_latencies.push_back(latency);

        if self.execution_latencies.len() > LATENCY_HISTORY_SIZE {
            self.execution_latencies.pop_front();
        }

        self.update_stats();
    }

    fn record_datafeed(&mut self, latency: Duration) {
        self.last_datafeed = Some(latency);
        self.datafeed_latencies.push_back(latency);

        if self.datafeed_latencies.len() > LATENCY_HISTORY_SIZE {
            self.datafeed_latencies.pop_front();
        }

        self.update_stats();
    }

    fn update_stats(&mut self) {
        if !self.execution_latencies.is_empty() {
            let sum: Duration = self.execution_latencies.iter().sum();
            self.avg_execution = sum / self.execution_latencies.len() as u32;

            let mut sorted: Vec<Duration> = self.execution_latencies.iter().cloned().collect();
            sorted.sort();
            let p99_idx = (sorted.len() as f64 * 0.99) as usize;
            self.p99_execution = sorted.get(p99_idx).cloned().unwrap_or(Duration::ZERO);
        }

        if !self.datafeed_latencies.is_empty() {
            let sum: Duration = self.datafeed_latencies.iter().sum();
            self.avg_datafeed = sum / self.datafeed_latencies.len() as u32;

            let mut sorted: Vec<Duration> = self.datafeed_latencies.iter().cloned().collect();
            sorted.sort();
            let p99_idx = (sorted.len() as f64 * 0.99) as usize;
            self.p99_datafeed = sorted.get(p99_idx).cloned().unwrap_or(Duration::ZERO);
        }
    }
}

fn scenario_color(scenario: MarketScenario) -> Color {
    match scenario {
        MarketScenario::Normal => Color::Green,
        MarketScenario::HighVolatility => Color::Yellow,
        MarketScenario::FlashCrash => Color::Red,
        MarketScenario::Recovery => Color::Cyan,
        MarketScenario::LiquidityCrisis => Color::Magenta,
    }
}

struct MarketStats {
    bid_volume: Decimal,
    ask_volume: Decimal,
    spread: Decimal,
    imbalance: f64,
    avg_trade_size: Decimal,
    vwap: Decimal,
}

struct App {
    order_book: OrderBook,
    trades: VecDeque<(Trade, Instant)>,
    events: VecDeque<(String, Instant)>,
    simulator: MarketSimulator,
    last_update: Instant,
    update_interval: Duration,
    total_trades: u64,
    total_volume: Decimal,
    paused: bool,
    price_history: VecDeque<Decimal>,
    last_trade_price: Option<Decimal>,
    last_trade_direction: Option<Side>,
    latency_metrics: LatencyMetrics,
    market_stats: MarketStats,
}

impl App {
    fn new() -> Self {
        Self {
            order_book: OrderBook::new(),
            trades: VecDeque::new(),
            events: VecDeque::new(),
            simulator: MarketSimulator::new(),
            last_update: Instant::now(),
            update_interval: Duration::from_millis(500),
            total_trades: 0,
            total_volume: Decimal::ZERO,
            paused: false,
            price_history: VecDeque::new(),
            last_trade_price: None,
            last_trade_direction: None,
            latency_metrics: LatencyMetrics::new(),
            market_stats: MarketStats {
                bid_volume: Decimal::ZERO,
                ask_volume: Decimal::ZERO,
                spread: Decimal::ZERO,
                imbalance: 0.0,
                avg_trade_size: Decimal::ZERO,
                vwap: Decimal::ZERO,
            },
        }
    }

    fn update_market_stats(&mut self) {
        let bid_levels = self.order_book.buy_levels(10);
        let ask_levels = self.order_book.sell_levels(10);

        self.market_stats.bid_volume = bid_levels.iter().map(|(_, qty)| *qty).sum();
        self.market_stats.ask_volume = ask_levels.iter().map(|(_, qty)| *qty).sum();

        if let (Some((bid, _)), Some((ask, _))) = (
            self.order_book.best_buy(),
            self.order_book.best_sell()
        ) {
            self.market_stats.spread = ask - bid;

            let total_vol = self.market_stats.bid_volume + self.market_stats.ask_volume;
            if total_vol > Decimal::ZERO {
                self.market_stats.imbalance =
                    ((self.market_stats.bid_volume - self.market_stats.ask_volume) / total_vol)
                    .to_f64().unwrap_or(0.0);
            }
        }
    }

    fn update(&mut self) {
        if self.paused || self.last_update.elapsed() < self.update_interval {
            return;
        }

        let start = Instant::now();
        let (side, price, quantity, id) = self.simulator.generate_order();

        let trades_result = self.order_book.place_order(side, price, quantity, id);

        match trades_result {
            Ok(trades) => {
                let latency = start.elapsed();
                self.latency_metrics.record_execution(latency);

                if trades.is_empty() {
                    self.events.push_front((
                        format!("{:?} order #{} added: {} @ {}", side, id, quantity, price),
                        Instant::now(),
                    ));
                } else {
                    for trade in &trades {
                        self.total_trades += 1;
                        self.total_volume += trade.quantity;

                        self.trades.push_front((trade.clone(), Instant::now()));
                        if self.trades.len() > MAX_TRADES {
                            self.trades.pop_back();
                        }

                        self.last_trade_price = Some(trade.price);
                        self.last_trade_direction = Some(side);

                        self.price_history.push_front(trade.price);
                        if self.price_history.len() > 50 {
                            self.price_history.pop_back();
                        }
                    }

                    self.events.push_front((
                        format!("{} trade(s) executed", trades.len()),
                        Instant::now(),
                    ));
                }
            }
            Err(e) => {
                self.events.push_front((
                    format!("Order failed: {}", e),
                    Instant::now(),
                ));
            }
        }

        if self.events.len() > MAX_EVENTS {
            self.events.pop_back();
        }

        self.update_market_stats();
        self.last_update = Instant::now();
    }

    fn get_book_levels(&self, side: Side, limit: usize) -> Vec<(Decimal, Decimal)> {
        match side {
            Side::Buy => self.order_book.buy_levels(limit),
            Side::Sell => self.order_book.sell_levels(limit),
        }
    }
}

fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = App::new();
    let res = run_app(&mut terminal, app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        app.update();
        terminal.draw(|f| ui(f, &app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char(' ') => app.paused = !app.paused,
                    KeyCode::Char('+') => {
                        app.update_interval = app.update_interval.saturating_sub(Duration::from_millis(100));
                    }
                    KeyCode::Char('-') => {
                        app.update_interval = (app.update_interval + Duration::from_millis(100)).min(Duration::from_secs(2));
                    }
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
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
        .constraints([Constraint::Percentage(30), Constraint::Percentage(40), Constraint::Percentage(30)])
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
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
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
            Span::styled("| ⏸ PAUSED", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        } else {
            Span::styled("| ▶ RUNNING", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        },
    ];

    let header = Paragraph::new(Line::from(header_text))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Blue)))
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
        _ => app.simulator.mid_price,
    };

    let sell_items: Vec<ListItem> = sell_levels
        .iter()
        .rev()
        .map(|(price, qty)| {
            let bar_width = 20;
            let bar_len = ((qty.to_f64().unwrap_or(0.0) * bar_width as f64) / max_qty.to_f64().unwrap_or(100.0)) as usize;
            let bar = "█".repeat(bar_len.min(bar_width));
            let padding = " ".repeat(bar_width - bar_len.min(bar_width));

            ListItem::new(format!(
                "{:>7.2} │ {:>8} │ {}{}",
                price, qty, bar, padding
            ))
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
            let bar_len = ((qty.to_f64().unwrap_or(0.0) * bar_width as f64) / max_qty.to_f64().unwrap_or(100.0)) as usize;
            let bar = "█".repeat(bar_len.min(bar_width));
            let padding = " ".repeat(bar_width - bar_len.min(bar_width));

            ListItem::new(format!(
                "{:>7.2} │ {:>8} │ {}{}",
                price, qty, bar, padding
            ))
            .style(Style::default().fg(Color::Green))
        })
        .collect();

    let buy_list = List::new(buy_items)
        .block(Block::default().title(format!("📈 Bids | Mid: {:.2}", mid_price)).borders(Borders::ALL))
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
            let age_str = if age < 1 { "now".to_string() } else { format!("{}s ago", age) };

            let color = if trade.maker_id % 2 == 0 {
                Color::LightGreen
            } else {
                Color::LightRed
            };

            ListItem::new(format!(
                "{:>8} @ {:>7.2} │ M:{:>4} T:{:>4} │ {}",
                trade.quantity,
                trade.price,
                trade.maker_id,
                trade.taker_id,
                age_str
            ))
            .style(Style::default().fg(color))
        })
        .collect();

    let trades_list = List::new(trades)
        .block(Block::default()
            .title(format!("💹 Recent Trades ({})", app.total_trades))
            .borders(Borders::ALL));

    f.render_widget(trades_list, area);
}

fn draw_stats(f: &mut Frame, area: Rect, app: &App) {
    let stats_text = vec![
        Line::from(vec![
            Span::raw("📊 "),
            Span::styled("Market Statistics", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(format!("Volume: {} | Avg Size: {:.2}",
            app.total_volume,
            if app.total_trades > 0 {
                app.total_volume / Decimal::from(app.total_trades)
            } else {
                Decimal::ZERO
            }
        )),
        Line::from(format!("Spread: {:.2} | Imbalance: {:.1}%",
            app.market_stats.spread,
            app.market_stats.imbalance * 100.0
        )),
        Line::from(format!("Bid Vol: {} | Ask Vol: {}",
            app.market_stats.bid_volume,
            app.market_stats.ask_volume
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
        .constraints([Constraint::Percentage(40), Constraint::Percentage(30), Constraint::Percentage(30)])
        .split(area);

    draw_price_chart(f, chunks[0], app);
    draw_latency_metrics(f, chunks[1], app);
    draw_events(f, chunks[2], app);
}

fn draw_price_chart(f: &mut Frame, area: Rect, app: &App) {
    if app.price_history.is_empty() {
        let empty = Paragraph::new("Waiting for trades...")
            .block(Block::default().title("📈 Price Chart").borders(Borders::ALL))
            .alignment(Alignment::Center);
        f.render_widget(empty, area);
        return;
    }

    let prices: Vec<(f64, f64)> = app.price_history
        .iter()
        .rev()
        .enumerate()
        .map(|(i, p)| (i as f64, p.to_f64().unwrap_or(0.0)))
        .collect();

    let min_price = app.price_history.iter().min().cloned().unwrap_or(dec!(0));
    let max_price = app.price_history.iter().max().cloned().unwrap_or(dec!(1000));
    let price_range = max_price - min_price;
    let y_min = (min_price - price_range * dec!(0.1)).to_f64().unwrap_or(0.0);
    let y_max = (max_price + price_range * dec!(0.1)).to_f64().unwrap_or(1000.0);

    let datasets = vec![Dataset::default()
        .name("Price")
        .marker(symbols::Marker::Braille)
        .style(Style::default().fg(Color::Cyan))
        .graph_type(GraphType::Line)
        .data(&prices)];

    let chart = Chart::new(datasets)
        .block(Block::default().title("📈 Price Chart").borders(Borders::ALL))
        .x_axis(
            Axis::default()
                .bounds([0.0, prices.len() as f64])
                .labels(vec![])
        )
        .y_axis(
            Axis::default()
                .bounds([y_min, y_max])
                .labels(vec![
                    format!("{:.0}", y_min).into(),
                    format!("{:.0}", y_max).into(),
                ])
        );

    f.render_widget(chart, area);
}

fn draw_latency_metrics(f: &mut Frame, area: Rect, app: &App) {
    let metrics_text = vec![
        Line::from(vec![
            Span::raw("⚡ "),
            Span::styled("Performance", Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(format!("Execution: {:?} | Avg: {:?}",
            app.latency_metrics.last_execution.unwrap_or(Duration::ZERO),
            app.latency_metrics.avg_execution
        )),
        Line::from(format!("P99 Exec: {:?} | P99 Feed: {:?}",
            app.latency_metrics.p99_execution,
            app.latency_metrics.p99_datafeed
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
            let age_str = if age < 1 { "now".to_string() } else { format!("{}s", age) };

            ListItem::new(format!("{} │ {}", age_str, msg))
                .style(Style::default().fg(Color::Gray))
        })
        .collect();

    let events_list = List::new(events)
        .block(Block::default().title("📝 Events").borders(Borders::ALL));

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
            Span::styled("Book Stats: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("Buy Levels: {} │ ", app.order_book.buy_depth())),
            Span::raw(format!("Sell Levels: {} │ ", app.order_book.sell_depth())),
            Span::raw(format!("Total Orders: {}",
                app.order_book.buy_depth() + app.order_book.sell_depth()
            )),
        ]),
    ];

    let footer = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);

    f.render_widget(footer, area);
}