use crate::tui;
use anyhow::{Context, Result};
use selena_core::Core;
use selena_core::config::{ConfigPaths, load_or_create};
use selena_core::runtime::{RuntimeInfo, process_is_alive, read_runtime, remove_runtime};
use selena_server::host;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing_subscriber::EnvFilter;

pub async fn run() -> Result<()> {
    let args = normalized_args();
    let command = args.get(1).map(String::as_str).unwrap_or("");
    match command {
        "" => run_tui_entry(args).await,
        "web" => run_web(args, true).await,
        "serve" => run_web(args, false).await,
        "desktop" => run_desktop(args),
        "status" => run_status().await,
        "doctor" => run_doctor().await,
        "config" => run_config().await,
        "version" | "--version" | "-v" => {
            println!("selena {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => {
            eprintln!("unknown command `{other}`\n");
            print_help();
            std::process::exit(2);
        }
    }
}

fn normalized_args() -> Vec<String> {
    let mut args: Vec<String> = std::env::args().collect();
    if let Some(command) = args.get_mut(1)
        && !command.starts_with('-')
    {
        *command = normalize_command(command);
    }
    args
}

fn normalize_command(command: &str) -> String {
    command.to_ascii_lowercase()
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|arg| arg == name)
}

async fn make_core() -> Result<(Arc<Core>, ConfigPaths)> {
    let paths = ConfigPaths::resolve()?;
    let config = load_or_create(&paths).await?;
    let core = Core::new(config, paths.clone()).await?;
    Ok((Arc::new(core), paths))
}

async fn check_existing(paths: &ConfigPaths) -> Option<RuntimeInfo> {
    let runtime = read_runtime(&paths.runtime_file)?;
    if !process_is_alive(runtime.pid) {
        return None;
    }
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/health", runtime.base_url()))
        .timeout(Duration::from_millis(800))
        .send()
        .await
        .ok()?;
    response.status().is_success().then_some(runtime)
}

async fn run_web(args: Vec<String>, open: bool) -> Result<()> {
    init_logging();
    let (core, paths) = make_core().await?;
    if let Some(runtime) = check_existing(&paths).await {
        println!("Selena is already running at {}", runtime.base_url());
        if open && !has_flag(&args, "--no-open") {
            let _ = webbrowser::open(&runtime.admin_url());
        }
        return Ok(());
    }

    let host_override = flag(&args, "--host").unwrap_or(core.config.host.clone());
    let port_override = flag(&args, "--port")
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(core.config.port);
    let prepared = host::prepare(core.clone(), &host_override, port_override).await?;
    let runtime = prepared.runtime.clone();
    println!("Selena listening at {}", runtime.base_url());
    println!("Admin: {}", runtime.admin_url());
    if open && !has_flag(&args, "--no-open") {
        let _ = webbrowser::open(&runtime.admin_url());
    }
    host::serve_prepared(prepared).await?;
    Ok(())
}

fn run_desktop(args: Vec<String>) -> Result<()> {
    let explicit = flag(&args, "--bin").map(PathBuf::from);
    let candidates = if let Some(explicit) = explicit {
        vec![explicit]
    } else {
        let mut candidates = Vec::new();
        if let Ok(value) = std::env::var("SELENA_DESKTOP_BIN") {
            candidates.push(PathBuf::from(value));
        }
        if let Ok(current) = std::env::current_exe()
            && let Some(parent) = current.parent()
        {
            candidates.push(parent.join(format!("selena-desktop{}", std::env::consts::EXE_SUFFIX)));
        }
        candidates.push(PathBuf::from(format!(
            "selena-desktop{}",
            std::env::consts::EXE_SUFFIX
        )));
        candidates
    };
    let binary = candidates
        .into_iter()
        .find(|candidate| candidate.exists())
        .or_else(|| {
            candidates_lookup()
                .into_iter()
                .find(|candidate| candidate.exists())
        });
    let Some(binary) = binary else {
        anyhow::bail!(
            "selena-desktop was not found. Install the Selena desktop bundle or set SELENA_DESKTOP_BIN."
        );
    };
    let mut command = std::process::Command::new(&binary);
    command.args(args.iter().skip(2));
    command
        .spawn()
        .with_context(|| format!("launch {}", binary.display()))?;
    Ok(())
}

fn candidates_lookup() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(path) = std::env::var("PATH") {
        for directory in path.split(std::path::MAIN_SEPARATOR) {
            let path = PathBuf::from(directory)
                .join(format!("selena-desktop{}", std::env::consts::EXE_SUFFIX));
            candidates.push(path);
        }
    }
    candidates
}

async fn run_status() -> Result<()> {
    let paths = ConfigPaths::resolve()?;
    match read_runtime(&paths.runtime_file) {
        Some(runtime) if process_is_alive(runtime.pid) => {
            println!("Selena {} is running", runtime.version);
            println!("PID:    {}", runtime.pid);
            println!("URL:    {}", runtime.base_url());
            println!("Admin:  {}", runtime.admin_url());
            println!("Home:   {}", paths.home.display());
        }
        _ => {
            println!("Selena is not running.");
            println!("Home: {}", paths.home.display());
        }
    }
    Ok(())
}

async fn run_doctor() -> Result<()> {
    let paths = ConfigPaths::resolve()?;
    println!("Selena doctor");
    println!("  version:       {}", env!("CARGO_PKG_VERSION"));
    println!("  home:          {}", paths.home.display());
    println!("  config:        {}", paths.config_file.display());
    println!("  database:      {}", paths.database_file.display());
    println!("  web assets:    {}", paths.web_dist.display());
    println!(
        "  database file: {}",
        if paths.database_file.exists() {
            "present"
        } else {
            "will be created"
        }
    );
    println!(
        "  desktop:       {}",
        find_desktop_binary()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "not installed".to_string())
    );
    if let Some(runtime) = check_existing(&paths).await {
        println!("  service:       running at {}", runtime.base_url());
    } else {
        println!("  service:       stopped");
    }
    Ok(())
}

fn find_desktop_binary() -> Option<PathBuf> {
    if let Ok(value) = std::env::var("SELENA_DESKTOP_BIN") {
        let path = PathBuf::from(value);
        if path.exists() {
            return Some(path);
        }
    }
    if let Ok(current) = std::env::current_exe()
        && let Some(parent) = current.parent()
    {
        let path = parent.join(format!("selena-desktop{}", std::env::consts::EXE_SUFFIX));
        if path.exists() {
            return Some(path);
        }
    }
    candidates_lookup().into_iter().find(|path| path.exists())
}

async fn run_config() -> Result<()> {
    let (core, paths) = make_core().await?;
    println!("home:      {}", paths.home.display());
    println!("config:    {}", paths.config_file.display());
    println!("database:  {}", paths.database_file.display());
    println!("master:    {}", paths.master_key_file.display());
    println!("runtime:   {}", paths.runtime_file.display());
    println!("host:      {}", core.config.host);
    println!("port:      {}", core.config.port);
    println!("theme:     {}", core.config.theme);
    Ok(())
}

async fn run_tui_entry(args: Vec<String>) -> Result<()> {
    let (core, paths) = make_core().await?;
    let existing = check_existing(&paths).await;
    let mut owned_task: Option<JoinHandle<()>> = None;
    let runtime = if let Some(runtime) = existing {
        runtime
    } else {
        let host_override = flag(&args, "--host").unwrap_or(core.config.host.clone());
        let port_override = flag(&args, "--port")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(core.config.port);
        let prepared = host::prepare(core.clone(), &host_override, port_override).await?;
        let runtime = prepared.runtime.clone();
        let task = tokio::spawn(async move {
            if let Err(error) = host::serve_prepared(prepared).await {
                tracing::error!("Selena server stopped: {error}");
            }
        });
        owned_task = Some(task);
        runtime
    };

    let result = tui::run(core.clone(), runtime.clone()).await;
    if let Some(task) = owned_task {
        task.abort();
        remove_runtime(&core.paths.runtime_file).await;
    }
    result
}

fn print_help() {
    println!(
        "Selena {}\n\
         \n\
         Usage:\n\
           selena                 Start the terminal control room\n\
           selena web             Start the web control room\n\
           selena desktop         Launch the desktop app\n\
           selena serve           Run the headless gateway\n\
           selena status          Show process and endpoint status\n\
           selena doctor          Diagnose installation\n\
           selena config          Show configuration paths\n\
           selena version         Print version\n\
         \n\
         Commands are case-insensitive: SELENA WEB, Selena Desktop.",
        env!("CARGO_PKG_VERSION")
    );
}

pub fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::normalize_command;

    #[test]
    fn command_names_are_case_insensitive() {
        assert_eq!(normalize_command("WEB"), "web");
        assert_eq!(normalize_command("Desktop"), "desktop");
        assert_eq!(normalize_command("SeRvE"), "serve");
    }
}
