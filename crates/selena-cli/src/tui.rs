use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs, Wrap};
use ratatui::{Frame, Terminal};
use selena_core::Core;
use selena_core::models::{ClientKey, Overview, Provider, RequestLogRow, Route};
use selena_core::runtime::RuntimeInfo;
use std::io::{IsTerminal, Stdout};
use std::time::{Duration, Instant};

const BG: Color = Color::Rgb(246, 241, 231);
const SURFACE: Color = Color::Rgb(255, 251, 243);
const INK: Color = Color::Rgb(33, 29, 25);
const MUTED: Color = Color::Rgb(123, 113, 102);
const CORAL: Color = Color::Rgb(217, 99, 79);
const TEAL: Color = Color::Rgb(44, 110, 104);
const LINE: Color = Color::Rgb(227, 216, 200);

const TABS: [&str; 6] = [
    "Overview",
    "Providers",
    "Routes",
    "Client keys",
    "Logs",
    "Settings",
];

#[derive(Default)]
struct TuiData {
    overview: Option<Overview>,
    providers: Vec<Provider>,
    routes: Vec<Route>,
    client_keys: Vec<ClientKey>,
    logs: Vec<RequestLogRow>,
    error: Option<String>,
}

struct App {
    tab: usize,
    runtime: RuntimeInfo,
    quit: bool,
    status: String,
}

pub async fn run(core: std::sync::Arc<Core>, runtime: RuntimeInfo) -> Result<()> {
    if !std::io::stdout().is_terminal() {
        let data = load(&core).await;
        println!("Selena {} — {}", runtime.version, runtime.base_url());
        if let Some(overview) = data.overview {
            println!(
                "providers: {}  routes: {}  requests today: {}  errors today: {}",
                overview.providers, overview.routes, overview.requests_today, overview.errors_today
            );
        }
        println!("web admin: {}", runtime.admin_url());
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = App {
        tab: 0,
        runtime,
        quit: false,
        status: "ready".to_string(),
    };
    let mut data = load(&core).await;
    let mut last_refresh = Instant::now();

    let result = event_loop(&mut terminal, &mut app, &mut data, &core, &mut last_refresh).await;
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

async fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    data: &mut TuiData,
    core: &std::sync::Arc<Core>,
    last_refresh: &mut Instant,
) -> Result<()> {
    loop {
        terminal.draw(|frame| render(frame, app, data))?;
        if event::poll(Duration::from_millis(250))?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => app.quit = true,
                KeyCode::Tab | KeyCode::Right => {
                    app.tab = (app.tab + 1) % TABS.len();
                }
                KeyCode::BackTab | KeyCode::Left => {
                    app.tab = (app.tab + TABS.len() - 1) % TABS.len();
                }
                KeyCode::Char('1') => app.tab = 0,
                KeyCode::Char('2') => app.tab = 1,
                KeyCode::Char('3') => app.tab = 2,
                KeyCode::Char('4') => app.tab = 3,
                KeyCode::Char('5') => app.tab = 4,
                KeyCode::Char('6') => app.tab = 5,
                KeyCode::Char('r') => {
                    *data = load(core).await;
                    *last_refresh = Instant::now();
                    app.status = "refreshed".to_string();
                }
                KeyCode::Char('o') => {
                    let _ = webbrowser::open(&app.runtime.admin_url());
                    app.status = "opened web console".to_string();
                }
                _ => {}
            }
        }
        if app.quit {
            break;
        }
        if last_refresh.elapsed() > Duration::from_secs(5) {
            *data = load(core).await;
            *last_refresh = Instant::now();
        }
    }
    Ok(())
}

async fn load(core: &std::sync::Arc<Core>) -> TuiData {
    let mut data = TuiData::default();
    match core.overview().await {
        Ok(overview) => data.overview = Some(overview),
        Err(error) => data.error = Some(error.to_string()),
    }
    match core.store.list_providers().await {
        Ok(providers) => data.providers = providers,
        Err(error) => data.error = Some(error.to_string()),
    }
    match core.store.list_routes().await {
        Ok(routes) => data.routes = routes,
        Err(error) => data.error = Some(error.to_string()),
    }
    match core.store.list_client_keys().await {
        Ok(keys) => data.client_keys = keys,
        Err(error) => data.error = Some(error.to_string()),
    }
    match core.store.list_request_logs(30, 0).await {
        Ok(logs) => data.logs = logs,
        Err(error) => data.error = Some(error.to_string()),
    }
    data
}

fn render(frame: &mut Frame, app: &App, data: &TuiData) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(BG).fg(INK)),
        area,
    );
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(8),
        Constraint::Length(1),
    ])
    .split(area);

    render_header(frame, chunks[0], app, data);
    render_tabs(frame, chunks[1], app);
    match app.tab {
        0 => render_overview(frame, chunks[2], data),
        1 => render_providers(frame, chunks[2], data),
        2 => render_routes(frame, chunks[2], data),
        3 => render_client_keys(frame, chunks[2], data),
        4 => render_logs(frame, chunks[2], data),
        5 => render_settings(frame, chunks[2], app, data),
        _ => {}
    }
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "  q",
            Style::default().fg(CORAL).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" quit   ", Style::default().fg(MUTED)),
        Span::styled("tab/←→", Style::default().fg(TEAL)),
        Span::styled(" switch   ", Style::default().fg(MUTED)),
        Span::styled("r", Style::default().fg(TEAL)),
        Span::styled(" refresh   ", Style::default().fg(MUTED)),
        Span::styled("o", Style::default().fg(TEAL)),
        Span::styled(" open web   ", Style::default().fg(MUTED)),
        Span::styled(format!("  {}", app.status), Style::default().fg(MUTED)),
    ]))
    .style(Style::default().bg(BG));
    frame.render_widget(footer, chunks[3]);
}

fn render_header(frame: &mut Frame, area: Rect, app: &App, data: &TuiData) {
    let status = if data.error.is_some() {
        "degraded"
    } else {
        "listening"
    };
    let line = Line::from(vec![
        Span::styled(
            "  Selena ",
            Style::default().fg(INK).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("v{}", env!("CARGO_PKG_VERSION")),
            Style::default().fg(MUTED),
        ),
        Span::styled(
            "   ● ",
            Style::default().fg(if data.error.is_some() { CORAL } else { TEAL }),
        ),
        Span::styled(
            status,
            Style::default().fg(if data.error.is_some() { CORAL } else { TEAL }),
        ),
        Span::styled("   ", Style::default()),
        Span::styled(app.runtime.base_url(), Style::default().fg(MUTED)),
    ]);
    let title = Paragraph::new(vec![line])
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(LINE)),
        )
        .style(Style::default().bg(BG));
    frame.render_widget(title, area);
}

fn render_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let tabs = Tabs::new(TABS.to_vec())
        .select(app.tab)
        .style(Style::default().fg(MUTED).bg(BG))
        .highlight_style(
            Style::default()
                .fg(CORAL)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
        )
        .divider(Span::styled("  ·  ", Style::default().fg(LINE)));
    frame.render_widget(tabs, area);
}

fn panel(title: &str) -> Block<'_> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(INK).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(LINE))
        .style(Style::default().bg(SURFACE).fg(INK))
}

fn render_overview(frame: &mut Frame, area: Rect, data: &TuiData) {
    let Some(overview) = &data.overview else {
        frame.render_widget(
            Paragraph::new("  No overview data yet. Add a provider in the web console.")
                .style(Style::default().fg(MUTED).bg(BG)),
            area,
        );
        return;
    };
    let rows = Layout::vertical([Constraint::Length(5), Constraint::Min(5)]).split(area);
    let metrics = Layout::horizontal([
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
    ])
    .split(rows[0]);
    metric(
        frame,
        metrics[0],
        "PROVIDERS",
        &overview.providers.to_string(),
        TEAL,
    );
    metric(
        frame,
        metrics[1],
        "REQUESTS / 24H",
        &overview.requests_today.to_string(),
        CORAL,
    );
    metric(
        frame,
        metrics[2],
        "ERRORS / 24H",
        &overview.errors_today.to_string(),
        CORAL,
    );
    metric(
        frame,
        metrics[3],
        "AVG LATENCY",
        &format!("{:.0} ms", overview.avg_latency_ms),
        TEAL,
    );

    let lines = data
        .logs
        .iter()
        .take(12)
        .map(|log| {
            let color = if log.status_code >= 400 { CORAL } else { TEAL };
            Line::from(vec![
                Span::styled(
                    format!("  {:<10}", short_time(log.ts)),
                    Style::default().fg(MUTED),
                ),
                Span::styled(
                    format!("{:<12}", log.ingress_protocol),
                    Style::default().fg(TEAL),
                ),
                Span::styled(
                    format!("{:<22}", truncate(&log.model_requested, 22)),
                    Style::default().fg(INK),
                ),
                Span::styled(
                    format!(
                        "{:<18}",
                        truncate(&log.provider_name.clone().unwrap_or_default(), 18)
                    ),
                    Style::default().fg(MUTED),
                ),
                Span::styled(
                    format!("{:>4}", log.status_code),
                    Style::default().fg(color),
                ),
                Span::styled(
                    format!("  {} ms", log.latency_ms),
                    Style::default().fg(MUTED),
                ),
            ])
        })
        .collect::<Vec<_>>();
    let body = if lines.is_empty() {
        Paragraph::new("  No requests yet. Copy an endpoint from the web console to begin.")
            .block(panel("Recent requests"))
            .style(Style::default().bg(SURFACE).fg(MUTED))
            .wrap(Wrap { trim: false })
    } else {
        Paragraph::new(lines)
            .block(panel("Recent requests"))
            .style(Style::default().bg(SURFACE).fg(INK))
    };
    frame.render_widget(body, rows[1]);
}

fn metric(frame: &mut Frame, area: Rect, label: &str, value: &str, color: Color) {
    let text = vec![
        Line::from(Span::styled(label, Style::default().fg(MUTED))),
        Line::from(Span::styled(
            format!(" {value}"),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )),
    ];
    frame.render_widget(
        Paragraph::new(text)
            .block(panel(""))
            .style(Style::default().bg(SURFACE)),
        area,
    );
}

fn render_providers(frame: &mut Frame, area: Rect, data: &TuiData) {
    if data.providers.is_empty() {
        frame.render_widget(
            Paragraph::new("  No providers. Open the web console and add a Chat, Responses, or Messages provider.")
                .block(panel("Providers"))
                .style(Style::default().bg(SURFACE).fg(MUTED))
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    let rows = data.providers.iter().map(|provider| {
        let color = if provider.enabled { TEAL } else { MUTED };
        Row::new(vec![
            Cell::from(provider.name.clone()),
            Cell::from(protocol_label(&provider.protocol)),
            Cell::from(provider.base_url.clone()),
            Cell::from(if provider.enabled {
                "enabled"
            } else {
                "disabled"
            }),
        ])
        .style(Style::default().fg(color).bg(SURFACE))
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(22),
            Constraint::Length(22),
            Constraint::Min(32),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["Provider", "Protocol", "Base URL", "State"])
            .style(Style::default().fg(MUTED).add_modifier(Modifier::BOLD)),
    )
    .block(panel("Providers"))
    .style(Style::default().bg(SURFACE));
    frame.render_widget(table, area);
}

fn render_routes(frame: &mut Frame, area: Rect, data: &TuiData) {
    if data.routes.is_empty() {
        frame.render_widget(
            Paragraph::new("  No routes. Add a model alias or use `provider/model` directly.")
                .block(panel("Routes"))
                .style(Style::default().bg(SURFACE).fg(MUTED))
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    let rows = data.routes.iter().map(|route| {
        let provider = data
            .providers
            .iter()
            .find(|provider| provider.id == route.provider_id)
            .map_or_else(
                || route.provider_id.clone(),
                |provider| provider.name.clone(),
            );
        Row::new(vec![
            Cell::from(route.model_alias.clone()),
            Cell::from(provider),
            Cell::from(route.upstream_model.clone()),
        ])
        .style(Style::default().fg(INK).bg(SURFACE))
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(30),
            Constraint::Length(32),
            Constraint::Min(24),
        ],
    )
    .header(
        Row::new(vec!["Model alias", "Provider", "Upstream model"])
            .style(Style::default().fg(MUTED).add_modifier(Modifier::BOLD)),
    )
    .block(panel("Routes"))
    .style(Style::default().bg(SURFACE));
    frame.render_widget(table, area);
}

fn render_client_keys(frame: &mut Frame, area: Rect, data: &TuiData) {
    if data.client_keys.is_empty() {
        frame.render_widget(
            Paragraph::new(
                "  No client API keys. Create one in the web console to protect the gateway.",
            )
            .block(panel("Client keys"))
            .style(Style::default().bg(SURFACE).fg(MUTED))
            .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    let rows = data.client_keys.iter().map(|key| {
        Row::new(vec![
            Cell::from(key.name.clone()),
            Cell::from(key.key_prefix.clone()),
            Cell::from(if key.enabled { "enabled" } else { "disabled" }),
            Cell::from(
                key.rate_limit_rpm
                    .map(|limit| format!("{limit} rpm"))
                    .unwrap_or_else(|| "unlimited".to_string()),
            ),
        ])
        .style(Style::default().fg(INK).bg(SURFACE))
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(28),
            Constraint::Length(24),
            Constraint::Length(12),
            Constraint::Min(18),
        ],
    )
    .header(
        Row::new(vec!["Name", "Prefix", "State", "Limit"])
            .style(Style::default().fg(MUTED).add_modifier(Modifier::BOLD)),
    )
    .block(panel("Client keys"))
    .style(Style::default().bg(SURFACE));
    frame.render_widget(table, area);
}

fn render_logs(frame: &mut Frame, area: Rect, data: &TuiData) {
    if data.logs.is_empty() {
        frame.render_widget(
            Paragraph::new("  No requests yet.")
                .block(panel("Request logs"))
                .style(Style::default().bg(SURFACE).fg(MUTED)),
            area,
        );
        return;
    }
    let rows = data.logs.iter().map(|log| {
        let color = if log.status_code >= 400 { CORAL } else { TEAL };
        Row::new(vec![
            Cell::from(short_time(log.ts)),
            Cell::from(log.ingress_protocol.clone()),
            Cell::from(truncate(&log.model_requested, 26)),
            Cell::from(log.provider_name.clone().unwrap_or_else(|| "—".to_string())),
            Cell::from(log.status_code.to_string()),
            Cell::from(format!("{} ms", log.latency_ms)),
        ])
        .style(Style::default().fg(color).bg(SURFACE))
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(16),
            Constraint::Length(28),
            Constraint::Length(20),
            Constraint::Length(8),
            Constraint::Min(12),
        ],
    )
    .header(
        Row::new(vec![
            "Time", "Ingress", "Model", "Provider", "Status", "Latency",
        ])
        .style(Style::default().fg(MUTED).add_modifier(Modifier::BOLD)),
    )
    .block(panel("Request logs"))
    .style(Style::default().bg(SURFACE));
    frame.render_widget(table, area);
}

fn render_settings(frame: &mut Frame, area: Rect, app: &App, data: &TuiData) {
    let lines = vec![
        Line::from(vec![
            Span::styled("  Endpoint      ", Style::default().fg(MUTED)),
            Span::styled(app.runtime.base_url(), Style::default().fg(TEAL)),
        ]),
        Line::from(vec![
            Span::styled("  Admin         ", Style::default().fg(MUTED)),
            Span::styled(app.runtime.admin_url(), Style::default().fg(CORAL)),
        ]),
        Line::from(vec![
            Span::styled("  Home          ", Style::default().fg(MUTED)),
            Span::styled(
                dirs::home_dir()
                    .map(|home| home.join(".selena").display().to_string())
                    .unwrap_or_else(|| "~/.selena".to_string()),
                Style::default().fg(INK),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Providers     ", Style::default().fg(MUTED)),
            Span::styled(
                format!("{} configured", data.providers.len()),
                Style::default().fg(INK),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Routes        ", Style::default().fg(MUTED)),
            Span::styled(
                format!("{} configured", data.routes.len()),
                Style::default().fg(INK),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Every protocol is both an ingress and a provider protocol.",
            Style::default().fg(MUTED),
        )),
        Line::from(Span::styled(
            "  Same-protocol traffic is passed through; cross-protocol traffic is converted.",
            Style::default().fg(MUTED),
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(panel("Settings"))
            .style(Style::default().bg(SURFACE).fg(INK))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn protocol_label(protocol: &str) -> &'static str {
    match protocol {
        "responses" => "Responses",
        "anthropic_messages" => "Messages",
        _ => "Chat Completions",
    }
}

fn short_time(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|time| time.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| "—".to_string())
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        format!(
            "{}…",
            value
                .chars()
                .take(max.saturating_sub(1))
                .collect::<String>()
        )
    }
}
