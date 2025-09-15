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
    mid_price: f64,
    volatility: f64,
    scenario: MarketScenario,
    scenario_timer: u32,
}

impl MarketSimulator {
    fn new() -> Self {
        Self {
            next_order_id: 1,
            mid_price: 1000.0,
            volatility: 0.5,
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

    fn generate_order(&mut self) -> (Side, u64, u64, u64) {
        let mut rng = rand::thread_rng();
        self.update_scenario();

        let (volatility_mult, aggressive_prob, size_mult) = match self.scenario {
            MarketScenario::Normal => (1.0, 0.3, 1.0),
            MarketScenario::HighVolatility => (3.0, 0.5, 1.5),
            MarketScenario::FlashCrash => (10.0, 0.8, 2.0),
            MarketScenario::Recovery => (0.5, 0.2, 0.8),
            MarketScenario::LiquidityCrisis => (5.0, 0.1, 0.3),
        };

        let price_change = rng.gen_range(-self.volatility..=self.volatility) * volatility_mult;
        self.mid_price += price_change;
        self.mid_price = self.mid_price.clamp(900.0, 1100.0);

        let is_aggressive = rng.gen_bool(aggressive_prob);
        let side = if rng.gen_bool(0.5) {
            Side::Buy
        } else {
            Side::Sell
        };

        let price = if is_aggressive {
            match side {
                Side::Buy => (self.mid_price + rng.gen_range(0.0..5.0)) as u64,
                Side::Sell => (self.mid_price - rng.gen_range(0.0..5.0)) as u64,
            }
        } else {
            match side {
                Side::Buy => (self.mid_price - rng.gen_range(1.0..10.0)) as u64,
                Side::Sell => (self.mid_price + rng.gen_range(1.0..10.0)) as u64,
            }
        };

        let base_quantity = rng.gen_range(1..=20) * 10;
        let quantity = (base_quantity as f64 * size_mult) as u64;

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
        while self.execution_latencies.len() > LATENCY_HISTORY_SIZE {
            self.execution_latencies.pop_front();
        }
        self.update_stats();
    }

    fn record_datafeed(&mut self, latency: Duration) {
        self.last_datafeed = Some(latency);
        self.datafeed_latencies.push_back(latency);
        while self.datafeed_latencies.len() > LATENCY_HISTORY_SIZE {
            self.datafeed_latencies.pop_front();
        }
        self.update_stats();
    }

    fn update_stats(&mut self) {
        if !self.execution_latencies.is_empty() {
            let sum: Duration = self.execution_latencies.iter().sum();
            self.avg_execution = sum / self.execution_latencies.len() as u32;

            let mut sorted: Vec<_> = self.execution_latencies.iter().cloned().collect();
            sorted.sort();
            let p99_idx = (sorted.len() as f64 * 0.99) as usize;
            self.p99_execution = sorted.get(p99_idx).cloned().unwrap_or(Duration::ZERO);
        }

        if !self.datafeed_latencies.is_empty() {
            let sum: Duration = self.datafeed_latencies.iter().sum();
            self.avg_datafeed = sum / self.datafeed_latencies.len() as u32;

            let mut sorted: Vec<_> = self.datafeed_latencies.iter().cloned().collect();
            sorted.sort();
            let p99_idx = (sorted.len() as f64 * 0.99) as usize;
            self.p99_datafeed = sorted.get(p99_idx).cloned().unwrap_or(Duration::ZERO);
        }
    }
}

struct MarketStats {
    bid_volume: u64,
    ask_volume: u64,
    spread: u64,
    imbalance: f64,
    avg_trade_size: u64,
    vwap: f64,
}

struct App {
    order_book: OrderBook,
    trades: VecDeque<(Trade, Instant)>,
    events: VecDeque<(String, Instant)>,
    simulator: MarketSimulator,
    last_update: Instant,
    update_interval: Duration,
    total_trades: u64,
    total_volume: u64,
    paused: bool,
    price_history: VecDeque<u64>,
    last_trade_price: Option<u64>,
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
            total_volume: 0,
            paused: false,
            price_history: VecDeque::new(),
            last_trade_price: None,
            last_trade_direction: None,
            latency_metrics: LatencyMetrics::new(),
            market_stats: MarketStats {
                bid_volume: 0,
                ask_volume: 0,
                spread: 0,
                imbalance: 0.0,
                avg_trade_size: 0,
                vwap: 0.0,
            },
        }
    }

    fn update_market_stats(&mut self) {
        let bid_levels = self.order_book.buy_levels(10);
        let ask_levels = self.order_book.sell_levels(10);

        self.market_stats.bid_volume = bid_levels.iter().map(|(_, qty)| qty).sum();
        self.market_stats.ask_volume = ask_levels.iter().map(|(_, qty)| qty).sum();

        if let (Some((bid, _)), Some((ask, _))) =
            (self.order_book.best_buy(), self.order_book.best_sell())
        {
            self.market_stats.spread = ask.saturating_sub(bid);
        }

        let total_volume = self.market_stats.bid_volume + self.market_stats.ask_volume;
        if total_volume > 0 {
            self.market_stats.imbalance = (self.market_stats.bid_volume as f64
                - self.market_stats.ask_volume as f64)
                / total_volume as f64;
        }

        if self.total_trades > 0 {
            self.market_stats.avg_trade_size = self.total_volume / self.total_trades;
        }

        if !self.trades.is_empty() {
            let mut price_volume_sum = 0u64;
            let mut volume_sum = 0u64;
            for (trade, _) in self.trades.iter().take(20) {
                price_volume_sum += trade.price * trade.quantity;
                volume_sum += trade.quantity;
            }
            if volume_sum > 0 {
                self.market_stats.vwap = price_volume_sum as f64 / volume_sum as f64;
            }
        }
    }

    fn tick(&mut self) {
        if self.paused || self.last_update.elapsed() < self.update_interval {
            return;
        }

        let (side, price, quantity, id) = self.simulator.generate_order();

        let event = format!(
            "place_order({side:?}, {price}, {quantity}, #{id}) called"
        );
        self.events.push_front((event, Instant::now()));

        let execution_start = Instant::now();
        let trades = self.order_book.place_order(side, price, quantity, id);
        let execution_latency = execution_start.elapsed();
        self.latency_metrics.record_execution(execution_latency);

        for trade in trades {
            self.total_trades += 1;
            self.total_volume += trade.quantity;
            self.trades.push_front((trade.clone(), Instant::now()));

            self.last_trade_price = Some(trade.price);
            self.last_trade_direction = Some(side);

            self.price_history.push_back(trade.price);
            while self.price_history.len() > 50 {
                self.price_history.pop_front();
            }

            let trade_event = format!(
                "→ Trade: {} @ {} (maker:#{}, taker:#{})",
                trade.quantity, trade.price, trade.maker_id, trade.taker_id
            );
            self.events.push_front((trade_event, Instant::now()));
        }

        while self.trades.len() > MAX_TRADES {
            self.trades.pop_back();
        }
        while self.events.len() > MAX_EVENTS {
            self.events.pop_back();
        }

        let datafeed_start = Instant::now();
        std::thread::sleep(Duration::from_micros(rand::thread_rng().gen_range(10..100)));
        let datafeed_latency = datafeed_start.elapsed();
        self.latency_metrics.record_datafeed(datafeed_latency);

        self.update_market_stats();

        self.last_update = Instant::now();
    }

    fn get_book_levels(&self, side: Side, limit: usize) -> Vec<(u64, u64)> {
        let mut levels = Vec::new();

        if side == Side::Buy {
            let current_best = self.order_book.best_buy();
            while levels.len() < limit && current_best.is_some() {
                levels.push(current_best.unwrap());
                break;
            }
        } else {
            let current_best = self.order_book.best_sell();
            while levels.len() < limit && current_best.is_some() {
                levels.push(current_best.unwrap());
                break;
            }
        }

        levels
    }
}

fn main() -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    let res = run_app(&mut terminal, &mut app);

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

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char(' ') => app.paused = !app.paused,
                    KeyCode::Char('+') => {
                        if app.update_interval > Duration::from_millis(100) {
                            app.update_interval -= Duration::from_millis(100);
                        }
                    }
                    KeyCode::Char('-') => {
                        if app.update_interval < Duration::from_secs(2) {
                            app.update_interval += Duration::from_millis(100);
                        }
                    }
                    _ => {}
                }
            }
        }

        app.tick();
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(20),
            Constraint::Length(3),
        ])
        .split(f.size());

    render_header(f, app, chunks[0]);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(35),
            Constraint::Percentage(35),
        ])
        .split(chunks[1]);

    render_order_book(f, app, main_chunks[0]);

    let middle_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(main_chunks[1]);

    render_api_calls(f, app, middle_chunks[0]);
    render_price_chart(f, app, middle_chunks[1]);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(10)])
        .split(main_chunks[2]);

    render_last_price(f, app, right_chunks[0]);
    render_trades(f, app, right_chunks[1]);

    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[2]);

    let status = if app.paused { "PAUSED" } else { "RUNNING" };
    let scenario_icon = match app.simulator.scenario {
        MarketScenario::Normal => "🟢",
        MarketScenario::HighVolatility => "🟡",
        MarketScenario::FlashCrash => "🔴",
        MarketScenario::Recovery => "🔵",
        MarketScenario::LiquidityCrisis => "⚠️",
    };
    let footer_text = format!(
        " {} {} | Speed: {}ms | Trades: {} | [Space] [+/-] [Q]",
        scenario_icon,
        status,
        app.update_interval.as_millis(),
        app.total_trades
    );
    let controls = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title(" Controls "));
    f.render_widget(controls, footer_chunks[0]);

    render_latency_metrics(f, app, footer_chunks[1]);
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let scenario_text = match app.simulator.scenario {
        MarketScenario::Normal => "Normal Market",
        MarketScenario::HighVolatility => "High Volatility",
        MarketScenario::FlashCrash => "FLASH CRASH",
        MarketScenario::Recovery => "Recovery Phase",
        MarketScenario::LiquidityCrisis => "Liquidity Crisis",
    };

    let scenario_color = match app.simulator.scenario {
        MarketScenario::Normal => Color::Green,
        MarketScenario::HighVolatility => Color::Yellow,
        MarketScenario::FlashCrash => Color::Red,
        MarketScenario::Recovery => Color::Blue,
        MarketScenario::LiquidityCrisis => Color::Magenta,
    };

    let imbalance = app.market_stats.imbalance;
    let imbalance_text = if imbalance > 0.2 {
        "▶▶▶ BUY"
    } else if imbalance < -0.2 {
        "SELL ◀◀◀"
    } else {
        "BALANCED"
    };

    let imbalance_color = if imbalance > 0.2 {
        Color::Green
    } else if imbalance < -0.2 {
        Color::Red
    } else {
        Color::Gray
    };

    let header_text = vec![
        Span::styled(
            "📈 Order Book ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            scenario_text,
            Style::default()
                .fg(scenario_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("Spread: {} ", app.market_stats.spread),
            Style::default().fg(Color::White),
        ),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled(imbalance_text, Style::default().fg(imbalance_color)),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("VWAP: {:.1}", app.market_stats.vwap),
            Style::default().fg(Color::Cyan),
        ),
    ];

    let header = Paragraph::new(Line::from(header_text))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, area);
}

fn render_order_book(f: &mut Frame, app: &App, area: Rect) {
    const DEPTH_DISPLAY: usize = 10;

    let block = Block::default()
        .title(" Order Book Depth ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL);

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let sell_levels = app.order_book.sell_levels(DEPTH_DISPLAY);
    let buy_levels = app.order_book.buy_levels(DEPTH_DISPLAY);

    let max_qty = sell_levels
        .iter()
        .chain(buy_levels.iter())
        .map(|(_, qty)| *qty)
        .max()
        .unwrap_or(100)
        .max(100);

    let bar_width = inner_area.width.saturating_sub(15) as usize;

    let mid_price = match (app.order_book.best_buy(), app.order_book.best_sell()) {
        (Some((bid, _)), Some((ask, _))) => (bid + ask) / 2,
        _ => 1000,
    };

    let mut lines = Vec::new();

    let mut sell_levels_sorted = sell_levels.clone();
    sell_levels_sorted.sort_by(|a, b| b.0.cmp(&a.0));

    for (price, qty) in sell_levels_sorted.iter().take(DEPTH_DISPLAY) {
        let bar_len = (*qty as usize * bar_width / max_qty as usize).min(bar_width);
        let bar = "█".repeat(bar_len);
        let spaces = " ".repeat(bar_width - bar_len);

        lines.push(Line::from(vec![
            Span::raw(format!("{price:>6} │ ")),
            Span::raw(spaces),
            Span::styled(bar, Style::default().fg(Color::Red)),
            Span::styled(format!(" {qty:>6}"), Style::default().fg(Color::LightRed)),
        ]));
    }

    for _ in sell_levels.len()..DEPTH_DISPLAY {
        lines.push(Line::from("       │"));
    }

    lines.push(Line::from(vec![Span::styled(
        format!(
            "  ──── │ {:^width$} │ ────",
            format!("SPREAD @ {}", mid_price),
            width = bar_width
        ),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )]));

    for (price, qty) in buy_levels.iter().take(DEPTH_DISPLAY) {
        let bar_len = (*qty as usize * bar_width / max_qty as usize).min(bar_width);
        let bar = "█".repeat(bar_len);
        let spaces = " ".repeat(bar_width - bar_len);

        lines.push(Line::from(vec![
            Span::raw(format!("{price:>6} │ ")),
            Span::raw(spaces),
            Span::styled(bar, Style::default().fg(Color::Green)),
            Span::styled(
                format!(" {qty:>6}"),
                Style::default().fg(Color::LightGreen),
            ),
        ]));
    }

    for _ in buy_levels.len()..DEPTH_DISPLAY {
        lines.push(Line::from("       │"));
    }

    let paragraph = Paragraph::new(lines).alignment(Alignment::Left);

    f.render_widget(paragraph, inner_area);
}

fn render_api_calls(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" API Calls & Returns ")
        .borders(Borders::ALL);

    let mut items: Vec<ListItem> = Vec::new();

    if let Some((price, qty)) = app.order_book.best_buy() {
        items.push(
            ListItem::new(format!("best_buy() → Some({price}, {qty})"))
                .style(Style::default().fg(Color::Green)),
        );
    } else {
        items.push(ListItem::new("best_buy() → None").style(Style::default().fg(Color::DarkGray)));
    }

    if let Some((price, qty)) = app.order_book.best_sell() {
        items.push(
            ListItem::new(format!("best_sell() → Some({price}, {qty})"))
                .style(Style::default().fg(Color::Red)),
        );
    } else {
        items.push(ListItem::new("best_sell() → None").style(Style::default().fg(Color::DarkGray)));
    }

    items.push(ListItem::new("─────────────────────"));

    for (event, time) in &app.events {
        let age = time.elapsed().as_secs_f32();
        let color = if age < 0.5 {
            Color::White
        } else if age < 1.0 {
            Color::Gray
        } else {
            Color::DarkGray
        };
        items.push(ListItem::new(event.clone()).style(Style::default().fg(color)));
    }

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_trades(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Recent Trades ")
        .borders(Borders::ALL);

    let mut items: Vec<ListItem> = Vec::new();

    for (trade, time) in &app.trades {
        let age = time.elapsed().as_secs_f32();
        let color = if age < 0.5 {
            Color::Yellow
        } else if age < 1.0 {
            Color::White
        } else {
            Color::Gray
        };

        let text = format!(
            "{:>6} @ {:>6} │ M:#{:<3} T:#{:<3}",
            trade.quantity, trade.price, trade.maker_id, trade.taker_id
        );
        items.push(ListItem::new(text).style(Style::default().fg(color)));
    }

    if items.is_empty() {
        items.push(ListItem::new("No trades yet...").style(Style::default().fg(Color::DarkGray)));
    }

    let list = List::new(items).block(block);
    f.render_widget(list, area);
}

fn render_price_chart(f: &mut Frame, app: &App, area: Rect) {
    if !app.price_history.is_empty() {
        let data: Vec<(f64, f64)> = app
            .price_history
            .iter()
            .enumerate()
            .map(|(i, &price)| (i as f64, price as f64))
            .collect();

        let min_price = app.price_history.iter().min().cloned().unwrap_or(900) as f64 - 5.0;
        let max_price = app.price_history.iter().max().cloned().unwrap_or(1100) as f64 + 5.0;

        let datasets = vec![Dataset::default()
            .name("Price")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Cyan))
            .data(&data)];

        let chart = Chart::new(datasets)
            .block(
                Block::default()
                    .title(" Price Chart (Last 50 Trades) ")
                    .borders(Borders::ALL),
            )
            .x_axis(
                Axis::default()
                    .bounds([0.0, app.price_history.len().max(1) as f64])
                    .labels(vec![]),
            )
            .y_axis(
                Axis::default()
                    .bounds([min_price, max_price])
                    .labels(vec![
                        Span::raw(format!("{min_price:.0}")),
                        Span::raw(format!("{:.0}", (min_price + max_price) / 2.0)),
                        Span::raw(format!("{max_price:.0}")),
                    ])
                    .style(Style::default().fg(Color::Gray)),
            );

        f.render_widget(chart, area);
    } else {
        let block = Block::default()
            .title(" Price Chart (Last 50 Trades) ")
            .borders(Borders::ALL);

        let text = Paragraph::new("Waiting for trades...")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center)
            .block(block);

        f.render_widget(text, area);
    }
}

fn render_latency_metrics(f: &mut Frame, app: &App, area: Rect) {
    let metrics = &app.latency_metrics;

    let exec_last = metrics
        .last_execution
        .map(|d| format!("{:.2}μs", d.as_micros()))
        .unwrap_or_else(|| "--".to_string());

    let feed_last = metrics
        .last_datafeed
        .map(|d| format!("{:.2}μs", d.as_micros()))
        .unwrap_or_else(|| "--".to_string());

    let exec_avg = if metrics.avg_execution > Duration::ZERO {
        format!("{:.2}μs", metrics.avg_execution.as_micros())
    } else {
        "--".to_string()
    };

    let feed_avg = if metrics.avg_datafeed > Duration::ZERO {
        format!("{:.2}μs", metrics.avg_datafeed.as_micros())
    } else {
        "--".to_string()
    };

    let exec_p99 = if metrics.p99_execution > Duration::ZERO {
        format!("{:.2}μs", metrics.p99_execution.as_micros())
    } else {
        "--".to_string()
    };

    let feed_p99 = if metrics.p99_datafeed > Duration::ZERO {
        format!("{:.2}μs", metrics.p99_datafeed.as_micros())
    } else {
        "--".to_string()
    };

    let text = format!(
        "Exec: {exec_last} (avg:{exec_avg} p99:{exec_p99}) | Feed: {feed_last} (avg:{feed_avg} p99:{feed_p99})"
    );

    let latency = Paragraph::new(text)
        .style(Style::default().fg(Color::Cyan))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Latency (μs) "),
        );
    f.render_widget(latency, area);
}

fn render_last_price(f: &mut Frame, app: &App, area: Rect) {
    let (price_text, color, arrow) = match app.last_trade_price {
        Some(price) => {
            let arrow = match app.last_trade_direction {
                Some(Side::Buy) => "↑",
                Some(Side::Sell) => "↓",
                None => " ",
            };
            let color = match app.last_trade_direction {
                Some(Side::Buy) => Color::Green,
                Some(Side::Sell) => Color::Red,
                None => Color::White,
            };
            (format!("{arrow} {price}"), color, true)
        }
        None => ("--".to_string(), Color::DarkGray, false),
    };

    let block = Block::default()
        .title(" Last Trade ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if arrow { color } else { Color::White }));

    let text = Paragraph::new(price_text)
        .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(block);

    f.render_widget(text, area);
}
