//! Authenticated client launch, child-process logging and executable lookup.
//! Flash can also run inside this process through `xenomit_flash.dll`.

use super::*;
use xenomit_game_api::{FlashInProcClient, FlashInProcConfig};

pub(super) struct LaunchResult {
    pub(super) session: Session,
    pub(super) statistics: SharedStats,
    pub(super) child: Option<Child>,
    pub(super) pid: Option<u32>,
    pub(super) bridge: Option<Arc<dyn GameClient>>,
    pub(super) client_logs: Arc<Mutex<VecDeque<String>>>,
    /// CDN-fetched map catalog; `None` when unreachable — the bundled
    /// `star_map` table remains the fallback.
    pub(super) star_catalog: Option<StarSystem>,
    /// The Flash host running inside this process, when in-process mode is on.
    pub(super) flash_host: Option<Arc<FlashInProcClient>>,
}

/// Collects the API launch settings consumed by the Flash launcher, plus the
/// performance FPS cap (`max_fps`, 0 = game default).
pub(super) fn game_api_options(config_tree: &serde_json::Value) -> serde_json::Value {
    let mut options = config_tree
        .pointer("/bot_settings/api_config")
        .cloned()
        .unwrap_or_default();
    if let (Some(map), Some(fps)) = (
        options.as_object_mut(),
        config_tree.pointer("/bot_settings/performance/max_fps"),
    ) {
        map.insert("max_fps".into(), fps.clone());
    }
    options
}

/// Whether Flash runs inside this process (InProcessPlayer model) instead of the
/// separate `xenomit_flash_client.exe`. `XENOMIT_FLASH_INPROC=1|0` overrides
/// the `api_config.flash_in_process` setting. Windows only.
pub(super) fn flash_in_process(api_options: &serde_json::Value) -> bool {
    if !cfg!(windows) {
        return false;
    }
    match std::env::var("XENOMIT_FLASH_INPROC").as_deref() {
        Ok("1") => true,
        Ok("0") => false,
        _ => api_options
            .get("flash_in_process")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    }
}

/// Flash launch options shared by the exe and the in-process host.
pub(super) struct FlashLaunchOptions {
    pub(super) width: u64,
    pub(super) height: u64,
    pub(super) quality: &'static str,
    pub(super) wmode: &'static str,
    pub(super) hidden: bool,
    pub(super) max_fps: u8,
}

impl FlashLaunchOptions {
    pub(super) fn from_api_options(api_options: &serde_json::Value) -> Self {
        let width = api_options
            .get("width")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1280)
            .clamp(320, 7680);
        let height = api_options
            .get("height")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(800)
            .clamp(240, 4320);
        let quality = match api_options
            .get("game_quality")
            .and_then(serde_json::Value::as_str)
        {
            Some("medium") => "medium",
            Some("high") => "high",
            _ => "low",
        };
        let wmode = if api_options
            .get("enforce_hw_accel")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true)
        {
            "direct"
        } else {
            "window"
        };
        let hidden = api_options
            .get("fully_hide_api")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true)
            && std::env::var("XENOMIT_FLASH_KEEP_VISIBLE").as_deref() != Ok("1");
        let max_fps = api_options
            .get("max_fps")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
            .min(100) as u8;
        Self {
            width,
            height,
            quality,
            wmode,
            hidden,
            max_fps,
        }
    }
}

type ClientLogs = Arc<Mutex<VecDeque<String>>>;

fn open_client_log() -> Result<(Arc<Mutex<File>>, ClientLogs), String> {
    let log_path = client_log_path();
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create the client log directory: {error}"))?;
    }
    let log_file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&log_path)
        .map_err(|error| format!("Could not open the client log file: {error}"))?;
    Ok((
        Arc::new(Mutex::new(log_file)),
        Arc::new(Mutex::new(VecDeque::new())),
    ))
}

pub(super) fn authenticate_and_launch(
    provider: Arc<dyn SessionProvider>,
    kind: ClientKind,
    backend: ClientBackend,
    flash_backend: FlashBackend,
    api_options: serde_json::Value,
    locale: String,
) -> Result<LaunchResult, String> {
    let session = provider.authenticate()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Could not initialize the network runtime: {error}"))?;
    runtime.block_on(async move {
        let statistics = Statistics::load(&session, statistics::default_directory());
        // Same config XMLs the game itself loads; failures fall back to the
        // bundled star_map table inside `resolve_map_name`/`portal_label`.
        let star_catalog = star_catalog::fetch(&session).await;
        match kind {
            ClientKind::Ruffle | ClientKind::Flash => {
                let mut launch = launch_game_client(
                    session,
                    kind,
                    backend,
                    flash_backend,
                    statistics,
                    api_options,
                    locale,
                )
                .await?;
                launch.star_catalog = star_catalog;
                Ok(launch)
            }
            ClientKind::Packet => Ok(LaunchResult {
                session: session.clone(),
                statistics: statistics.clone(),
                child: None,
                pid: None,
                bridge: Some(packet_client::connect(session, statistics).await?),
                client_logs: Arc::new(Mutex::new(VecDeque::new())),
                star_catalog,
                flash_host: None,
            }),
        }
    })
}

pub(super) async fn launch_game_client(
    session: Session,
    kind: ClientKind,
    backend: ClientBackend,
    flash_backend: FlashBackend,
    statistics: SharedStats,
    api_options: serde_json::Value,
    locale: String,
) -> Result<LaunchResult, String> {
    if kind == ClientKind::Flash && flash_in_process(&api_options) {
        return launch_flash_in_process(session, flash_backend, statistics, api_options, locale);
    }
    let executable = if kind == ClientKind::Flash {
        flash_client_executable()?
    } else {
        client_executable()?
    };
    let (log_file, client_logs) = open_client_log()?;
    record_client_log(
        &client_logs,
        &log_file,
        "controller",
        &format!("Starting game client from {}", executable.display()),
    );

    let launch_input = if kind == ClientKind::Flash {
        flash_launch_input(&session, &api_options, &locale)?
    } else {
        session.sid.clone()
    };
    let mut command = Command::new(&executable);
    if kind == ClientKind::Flash {
        command.arg("--backend").arg(if cfg!(windows) {
            if flash_backend == FlashBackend::NativeMemory {
                "native"
            } else {
                "cached"
            }
        } else {
            "native"
        });
        let options = FlashLaunchOptions::from_api_options(&api_options);
        command
            .arg("--width")
            .arg(options.width.to_string())
            .arg("--height")
            .arg(options.height.to_string())
            .arg("--quality")
            .arg(options.quality)
            .arg("--wmode")
            .arg(options.wmode);
        command.arg("--launch-stdin");
        if cfg!(windows) {
            command.arg("--name").arg("Xenomit Flash Client");
            if options.hidden {
                command.args(["--hidden", "--hidden-bootstrap", "draw"]);
            }
            if options.max_fps > 0 {
                command.arg("--max-fps").arg(options.max_fps.to_string());
            }
            if let Some(ocx) = std::env::var_os("XENOMIT_FLASH_OCX_PATH") {
                let ocx = PathBuf::from(ocx);
                if !ocx.is_file() {
                    return Err(format!(
                        "XENOMIT_FLASH_OCX_PATH is not a file: {}",
                        ocx.display()
                    ));
                }
                command.arg("--flash").arg(ocx);
            }
        } else {
            command
                .arg("--parent-pid")
                .arg(std::process::id().to_string());
            if api_options
                .get("fully_hide_api")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true)
                && std::env::var("XENOMIT_FLASH_KEEP_VISIBLE").as_deref() != Ok("1")
            {
                command.arg("--hidden");
            }
            if let Some(plugin) = std::env::var_os("XENOMIT_FLASH_PLUGIN_PATH") {
                let plugin = PathBuf::from(plugin);
                if !plugin.is_file() {
                    return Err(format!(
                        "XENOMIT_FLASH_PLUGIN_PATH is not a file: {}",
                        plugin.display()
                    ));
                }
                command.arg("--flash").arg(plugin);
            }
        }
    } else {
        command
            .arg("--domain")
            .arg(&session.server)
            .arg("--script-timeout")
            .arg("600")
            .arg("--smoke-pool")
            .arg("50")
            .arg("--session-stdin");
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("Could not launch {}: {error}", executable.display()))?;
    let pid = child.id();
    record_client_log(
        &client_logs,
        &log_file,
        "controller",
        &format!("Game client started with PID {pid}"),
    );
    if let Some(stdout) = child.stdout.take() {
        if let Err(error) = spawn_client_log_reader(
            stdout,
            "stdout",
            Arc::clone(&client_logs),
            Arc::clone(&log_file),
            statistics.clone(),
        ) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Could not capture game client output: {error}"));
        }
    }
    if let Some(stderr) = child.stderr.take() {
        if let Err(error) = spawn_client_log_reader(
            stderr,
            "stderr",
            Arc::clone(&client_logs),
            Arc::clone(&log_file),
            statistics.clone(),
        ) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Could not capture game client output: {error}"));
        }
    }
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        if let Err(error) = stdin.write_all(launch_input.as_bytes()) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "Could not pass the session to the game client: {error}"
            ));
        }
    }
    let bridge = match if kind == ClientKind::Flash {
        if cfg!(windows) && flash_backend != FlashBackend::NativeMemory {
            xenomit_game_api::connect_flash(pid)
        } else {
            xenomit_game_api::connect_flash_native(pid)
        }
    } else {
        xenomit_game_api::connect(pid, backend)
    } {
        Ok(bridge) => bridge,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Could not create the game API adapter: {error}"));
        }
    };
    Ok(LaunchResult {
        session,
        statistics,
        child: Some(child),
        pid: Some(pid),
        bridge: Some(bridge),
        client_logs,
        star_catalog: None,
        flash_host: None,
    })
}

/// Start Flash inside this process through `xenomit_flash.dll`: the same
/// bootstrap records and options as the exe, passed in memory. The host runs
/// on its own STA thread; there is no child process or pipe.
fn launch_flash_in_process(
    session: Session,
    flash_backend: FlashBackend,
    statistics: SharedStats,
    api_options: serde_json::Value,
    locale: String,
) -> Result<LaunchResult, String> {
    let (log_file, client_logs) = open_client_log()?;
    let bootstrap = flash_bootstrap(&session, &api_options, &locale)?;
    let options = FlashLaunchOptions::from_api_options(&api_options);
    record_client_log(
        &client_logs,
        &log_file,
        "controller",
        "Starting the in-process Flash host (xenomit_flash.dll)",
    );
    {
        let client_logs = Arc::clone(&client_logs);
        let log_file = Arc::clone(&log_file);
        let statistics = statistics.clone();
        xenomit_game_api::set_flash_log_sink(Some(Arc::new(move |level, line| {
            if let Ok(mut stats) = statistics.lock() {
                stats.client_line(line);
            }
            let source = if level == 0 { "flash" } else { "flash-error" };
            record_client_log(&client_logs, &log_file, source, line);
        })));
    }
    let ocx_path = match std::env::var_os("XENOMIT_FLASH_OCX_PATH") {
        Some(ocx) => {
            let ocx = PathBuf::from(ocx);
            if !ocx.is_file() {
                return Err(format!(
                    "XENOMIT_FLASH_OCX_PATH is not a file: {}",
                    ocx.display()
                ));
            }
            Some(ocx.to_string_lossy().into_owned())
        }
        None => None,
    };
    let config = FlashInProcConfig {
        movie: bootstrap.movie,
        base_url: bootstrap.base_url,
        sid: session.sid.clone(),
        flash_vars: bootstrap.flash_vars,
        width: bootstrap.width as i32,
        height: bootstrap.height as i32,
        backend: if flash_backend == FlashBackend::NativeMemory {
            "native"
        } else {
            "cached"
        }
        .into(),
        quality: Some(options.quality.into()),
        wmode: Some(options.wmode.into()),
        ocx_path,
        title: Some("Xenomit Flash Client".into()),
        hidden: options.hidden,
        hidden_draw_bootstrap: options.hidden,
        max_fps: options.max_fps,
        ..FlashInProcConfig::default()
    };
    let host = xenomit_game_api::start_flash_inproc(&config)
        .map_err(|error| format!("Could not start the in-process Flash host: {error}"))?;
    record_client_log(
        &client_logs,
        &log_file,
        "controller",
        "In-process Flash host started",
    );
    let bridge: Arc<dyn GameClient> = host.clone();
    Ok(LaunchResult {
        session,
        statistics,
        child: None,
        // The host shares this process; the PID keeps the window controls
        // and the account panel populated.
        pid: Some(std::process::id()),
        bridge: Some(bridge),
        client_logs,
        star_catalog: None,
        flash_host: Some(host),
    })
}

pub(super) fn spawn_client_log_reader<R>(
    stream: R,
    name: &'static str,
    client_logs: Arc<Mutex<VecDeque<String>>>,
    log_file: Arc<Mutex<File>>,
    statistics: SharedStats,
) -> std::io::Result<()>
where
    R: std::io::Read + Send + 'static,
{
    std::thread::Builder::new()
        .name(format!("xenomit-client-{name}"))
        .spawn(move || {
            let mut reader = BufReader::new(stream);
            let mut bytes = Vec::new();
            loop {
                bytes.clear();
                match reader.read_until(b'\n', &mut bytes) {
                    Ok(0) => break,
                    Ok(_) => {
                        let line = String::from_utf8_lossy(&bytes);
                        if let Ok(mut stats) = statistics.lock() {
                            stats.client_line(line.trim_end());
                        }
                        record_client_log(&client_logs, &log_file, name, line.trim_end());
                    }
                    Err(error) => {
                        record_client_log(
                            &client_logs,
                            &log_file,
                            "controller",
                            &format!("Could not read client {name}: {error}"),
                        );
                        break;
                    }
                }
            }
        })?;
    Ok(())
}

pub(super) fn record_client_log(
    client_logs: &Arc<Mutex<VecDeque<String>>>,
    log_file: &Arc<Mutex<File>>,
    source: &str,
    line: &str,
) {
    let message = format!("[{source}] {}", redact_client_log(line));
    if let Ok(mut file) = log_file.lock() {
        let _ = writeln!(file, "{message}");
        let _ = file.flush();
    }
    if let Ok(mut lines) = client_logs.lock() {
        if lines.len() >= 500 {
            lines.pop_front();
        }
        lines.push_back(message);
    }
}

pub(super) fn redact_client_log(line: &str) -> String {
    line.split_whitespace()
        .map(|word| {
            let lower = word.to_ascii_lowercase();
            if word.len() > 160
                || lower.contains("://")
                || lower.contains("dosid=")
                || lower.contains("sessionid=")
                || lower.contains("sid=")
                || lower.contains("token=")
            {
                "<redacted>"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The authenticated bootstrap stays on stdin, including SID and FlashVars.
pub(super) fn flash_launch_input(
    session: &Session,
    api_options: &serde_json::Value,
    locale: &str,
) -> Result<String, String> {
    let bootstrap = flash_bootstrap(session, api_options, locale)?;
    Ok(format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n",
        bootstrap.movie,
        bootstrap.base_url,
        session.sid,
        bootstrap.flash_vars,
        bootstrap.width,
        bootstrap.height
    ))
}

/// The Flash bootstrap records apart from the SID, which callers take from
/// the session so it is copied only where it is needed.
pub(super) struct FlashBootstrap {
    pub(super) movie: String,
    pub(super) base_url: String,
    pub(super) flash_vars: String,
    pub(super) width: u64,
    pub(super) height: u64,
}

pub(super) fn flash_bootstrap(
    session: &Session,
    api_options: &serde_json::Value,
    locale: &str,
) -> Result<FlashBootstrap, String> {
    let movie = session
        .preloader_url
        .as_deref()
        .ok_or("Flash preloader URL missing")?;
    let mut vars = session.flashvars.clone();
    vars.insert("autoStartEnabled".into(), "1".into());
    let use_3d = api_options
        .get("use_3d")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    vars.insert("display2d".into(), if use_3d { "1" } else { "2" }.into());
    if api_options
        .get("force_game_language")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
        && !locale.is_empty()
    {
        vars.insert("lang".into(), locale.to_owned());
    }
    let width = api_options
        .get("width")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| session.stage_width.max(1280) as u64)
        .clamp(320, 7680);
    let height = api_options
        .get("height")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| session.stage_height.max(800) as u64)
        .clamp(240, 4320);
    let mut pairs: Vec<_> = vars.iter().collect();
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    let vars = pairs
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");
    let instance = session.instance();
    let records = [movie, &instance, &session.sid, &vars];
    if records
        .iter()
        .any(|s| s.contains(['\n', '\r', '\0']) || s.len() > 65535)
    {
        return Err("Flash bootstrap contains an invalid record".into());
    }
    Ok(FlashBootstrap {
        movie: movie.to_owned(),
        base_url: instance,
        flash_vars: vars,
        width,
        height,
    })
}

pub(super) fn flash_client_executable() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("XENOMIT_FLASH_CLIENT_EXE") {
        let path = PathBuf::from(path);
        return if path.is_file() {
            Ok(path)
        } else {
            Err(format!(
                "XENOMIT_FLASH_CLIENT_EXE is not a file: {}",
                path.display()
            ))
        };
    }
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    if cfg!(windows) {
        let beside = current.with_file_name("xenomit_flash_client.exe");
        let sibling = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("xenomit-flash-client/build/xenomit_flash_client.exe");
        for path in [&beside, &sibling] {
            if path.is_file() {
                return Ok(path.clone());
            }
        }
        return Err(format!(
            "Flash client not found. Run {} or set XENOMIT_FLASH_CLIENT_EXE.",
            sibling
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("build.ps1")
                .display()
        ));
    }
    let beside = current.with_file_name("xenomit_flash_client");
    let sibling = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("xenomit-flash-client-linux/xenomit_flash_client");
    for path in [&beside, &sibling] {
        if path.is_file() {
            return Ok(path.clone());
        }
    }
    Err(format!(
        "Flash client not found. Build {} or set XENOMIT_FLASH_CLIENT_EXE.",
        sibling.parent().unwrap().join("scripts/build.sh").display()
    ))
}

pub(super) fn client_executable() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("XENOMIT_CLIENT_EXE") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "XENOMIT_CLIENT_EXE does not point to a file: {}",
            path.display()
        ));
    }
    let name = if cfg!(windows) {
        "xenomit_client.exe"
    } else {
        "xenomit_client"
    };
    let beside_controller = std::env::current_exe()
        .map_err(|error| error.to_string())?
        .with_file_name(name);
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let sibling_build = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("xenomit-client")
        .join("target")
        .join(profile)
        .join(name);
    for candidate in [&beside_controller, &sibling_build] {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }
    Err(format!(
        "Game client not found beside the controller or at {}. Build `xenomit_client` or set XENOMIT_CLIENT_EXE.",
        sibling_build.display()
    ))
}

pub(super) fn client_log_path() -> PathBuf {
    settings_path()
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("client.log")
}

impl BotView {
    pub(super) fn open_client_log(&mut self, cx: &mut Context<Self>) {
        let path = client_log_path();
        #[cfg(windows)]
        let result = Command::new("notepad.exe").arg(&path).spawn();
        #[cfg(not(windows))]
        let result: std::io::Result<Child> = Err(std::io::Error::other(
            "opening the client log is supported on Windows only",
        ));

        self.activity = match result {
            Ok(_) => format!("Opened client log: {}", path.display()),
            Err(error) => format!("Could not open client log: {error}"),
        };
        cx.notify();
    }

    /// Open `%APPDATA%\XenomitBot` — the folder holding modules.json, the
    /// client log and the statistics journals — in the file explorer.
    pub(super) fn open_settings_folder(&mut self, cx: &mut Context<Self>) {
        let path = self
            .config_path
            .parent()
            .map(|parent| parent.to_path_buf())
            .unwrap_or_else(|| self.config_path.clone());
        #[cfg(windows)]
        let result = Command::new("explorer.exe").arg(&path).spawn();
        #[cfg(not(windows))]
        let result: std::io::Result<Child> = Err(std::io::Error::other(
            "opening the settings folder is supported on Windows only",
        ));
        self.activity = match result {
            Ok(_) => format!("Opened settings folder: {}", path.display()),
            Err(error) => format!("Could not open the settings folder: {error}"),
        };
        cx.notify();
    }
}
