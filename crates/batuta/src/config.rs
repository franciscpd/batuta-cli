use crate::{
    cli::{Cli, DaemonArg},
    workspace::WorkspaceSelectorSource,
};
use batuta_tui::{
    app::{ColorDepthMode, ColorMode, Preset, Settings as TuiSettings, ThemeMode, UiSettings},
    keymap::{self, Keymap},
    theme::ColorDepth,
};
use serde::Deserialize;
use std::{
    env,
    ffi::OsString,
    fmt, fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ConfigFile {
    #[serde(default)]
    pub preset: PresetFile,
    #[serde(default)]
    pub daemon: DaemonFile,
    #[serde(default)]
    pub ui: UiFile,
    /// `[keys]` remaps Global/Lists actions (v1 scope). Values may be a
    /// single combo string or an array of combo strings. Kept as a raw
    /// table (rather than a typed struct) because action names are
    /// data-driven — see `batuta_tui::keymap::action_by_name`.
    #[serde(default)]
    pub keys: toml::Table,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PresetFile {
    pub agent: Option<String>,
    #[serde(rename = "loop")]
    pub loop_name: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct DaemonFile {
    pub transport: Option<String>,
    pub tcp_addr: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct UiFile {
    pub color: Option<String>,
    pub theme: Option<String>,
    pub color_depth: Option<String>,
    pub fps: Option<u16>,
    pub sessions_limit: Option<u64>,
    pub runs_limit: Option<u64>,
    pub notify: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DaemonCfg {
    pub transport: DaemonArg,
    pub tcp_addr: String,
}

#[derive(Clone, Debug)]
pub struct Settings {
    pub preset: Preset,
    pub daemon: DaemonCfg,
    pub ui: UiSettings,
    pub workspace: Option<String>,
    pub workspace_source: Option<WorkspaceSelectorSource>,
    pub config_path: PathBuf,
    pub loaded: bool,
    pub warnings: Vec<String>,
    pub keymap: Keymap,
}

impl Settings {
    pub fn tui_settings(&self, workspace: Option<batuta_tui::app::WorkspaceRef>) -> TuiSettings {
        TuiSettings {
            preset: self.preset.clone(),
            ui: self.ui.clone(),
            workspace,
            keymap: self.keymap.clone(),
        }
    }

    pub fn apply_to_cli(&self, cli: &mut Cli) {
        cli.workspace = self.workspace.clone();
        cli.daemon = Some(self.daemon.transport);
        cli.tcp_addr = Some(self.daemon.tcp_addr.clone());
    }
}

#[derive(Clone, Debug, Default)]
pub struct Env {
    pub workspace: Option<String>,
    pub no_color: bool,
}

impl Env {
    pub fn current() -> Self {
        Self {
            workspace: env::var("COMPOZY_WORKSPACE").ok(),
            no_color: env::var_os("NO_COLOR").is_some(),
        }
    }
}

#[derive(Debug)]
pub struct ConfigError {
    pub path: PathBuf,
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "config: {}:{}: {}",
            self.path.display(),
            self.line,
            self.message
        )
    }
}

pub fn default_path() -> PathBuf {
    default_path_from(env::var_os("XDG_CONFIG_HOME"), env::var_os("HOME"))
}

fn default_path_from(xdg_config_home: Option<OsString>, home: Option<OsString>) -> PathBuf {
    xdg_config_home
        .map(PathBuf::from)
        .or_else(|| home.map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("batuta/config.toml")
}

pub fn path(cli: &Cli) -> PathBuf {
    cli.config.clone().unwrap_or_else(default_path)
}

pub fn load(config_path: PathBuf) -> Result<(Option<ConfigFile>, bool, Vec<String>), ConfigError> {
    let source = match fs::read_to_string(&config_path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((None, false, vec![]));
        }
        Err(error) => {
            return Err(ConfigError {
                path: config_path,
                line: 1,
                message: error.to_string(),
            });
        }
    };
    let value = toml::from_str::<toml::Value>(&source)
        .map_err(|error| parse_error(&config_path, &source, error))?;
    let warnings = unknown_key_warnings(&value);
    let file = value
        .try_into::<ConfigFile>()
        .map_err(|error| ConfigError {
            path: config_path,
            line: 1,
            message: error.to_string(),
        })?;
    Ok((Some(file), true, warnings))
}

pub fn load_and_resolve(cli: &Cli, env: &Env) -> Result<Settings, ConfigError> {
    let config_path = path(cli);
    let (file, loaded, warnings) = load(config_path.clone())?;
    let mut settings = resolve(cli, env, file)?;
    settings.config_path = config_path;
    settings.loaded = loaded;
    settings.warnings.extend(warnings);
    Ok(settings)
}

pub fn resolve(cli: &Cli, env: &Env, file: Option<ConfigFile>) -> Result<Settings, ConfigError> {
    let file = file.unwrap_or_default();
    if file
        .daemon
        .transport
        .as_deref()
        .is_some_and(|value| parse_transport(Some(value)).is_none())
    {
        return Err(ConfigError {
            path: path(cli),
            line: 1,
            message: "daemon.transport must be auto, uds, or tcp".into(),
        });
    }
    if file
        .ui
        .color
        .as_deref()
        .is_some_and(|value| parse_color(Some(value)).is_none())
    {
        return Err(ConfigError {
            path: path(cli),
            line: 1,
            message: "ui.color must be auto or never".into(),
        });
    }
    if file
        .ui
        .theme
        .as_deref()
        .is_some_and(|value| parse_theme(Some(value)).is_none())
    {
        return Err(ConfigError {
            path: path(cli),
            line: 1,
            message: "ui.theme must be auto, dark, or light".into(),
        });
    }
    let preset = Preset {
        agent: file.preset.agent.unwrap_or_else(|| "batuta".into()),
        loop_name: file
            .preset
            .loop_name
            .unwrap_or_else(|| "batuta-deliver".into()),
        provider: file.preset.provider.unwrap_or_else(|| "claude".into()),
        model: file.preset.model.unwrap_or_default(),
    };
    let transport = cli
        .daemon
        .or_else(|| parse_transport(file.daemon.transport.as_deref()))
        .unwrap_or(DaemonArg::Auto);
    let tcp_addr = cli
        .tcp_addr
        .clone()
        .or(file.daemon.tcp_addr)
        .unwrap_or_else(|| "localhost:2123".into());
    let mut warnings = Vec::new();
    let color = if env.no_color {
        ColorMode::Never
    } else {
        parse_color(file.ui.color.as_deref()).unwrap_or(ColorMode::Auto)
    };
    let theme = parse_theme(file.ui.theme.as_deref()).unwrap_or(ThemeMode::Auto);
    let color_depth = match file.ui.color_depth.as_deref() {
        None => ColorDepthMode::Auto,
        Some(value) => parse_color_depth(Some(value)).unwrap_or_else(|| {
            warnings.push(format!(
                "config: ui.color_depth={value} invalid, using auto"
            ));
            ColorDepthMode::Auto
        }),
    };
    let fps = clamp(file.ui.fps.unwrap_or(30), 5, 60, "fps", &mut warnings);
    let sessions_limit = clamp(
        file.ui.sessions_limit.unwrap_or(50),
        1,
        100,
        "sessions_limit",
        &mut warnings,
    );
    let runs_limit = clamp(
        file.ui.runs_limit.unwrap_or(50),
        1,
        100,
        "runs_limit",
        &mut warnings,
    );
    let notify = file.ui.notify.unwrap_or(true);
    let keymap = resolve_keymap(file.keys, &mut warnings);
    let (workspace, workspace_source) = match (
        cli.workspace.as_deref().filter(|value| !value.is_empty()),
        env.workspace.as_deref().filter(|value| !value.is_empty()),
    ) {
        (Some(value), _) => (Some(value.to_owned()), Some(WorkspaceSelectorSource::Flag)),
        (None, Some(value)) => (
            Some(value.to_owned()),
            Some(WorkspaceSelectorSource::Environment),
        ),
        (None, None) => (None, None),
    };
    Ok(Settings {
        preset,
        daemon: DaemonCfg {
            transport,
            tcp_addr,
        },
        ui: UiSettings {
            color,
            theme,
            color_depth,
            fps,
            sessions_limit,
            runs_limit,
            notify,
        },
        workspace,
        workspace_source,
        config_path: path(cli),
        loaded: false,
        warnings,
        keymap,
    })
}

/// Resolves the `[keys]` table into a `Keymap`, starting from
/// `Keymap::default()` and applying each entry as a `rebind`. Unknown
/// action names and unparsable combo specs warn and are skipped rather
/// than failing config load; a rebind that steals a combo from another
/// action's only binding also warns (see `Keymap::rebind`).
fn resolve_keymap(keys: toml::Table, warnings: &mut Vec<String>) -> Keymap {
    let mut keymap = Keymap::default();
    for (name, value) in keys {
        let Some(action) = keymap::action_by_name(&name) else {
            warnings.push(format!("config: unknown key action `{name}`"));
            continue;
        };
        let specs: Vec<String> = match value {
            toml::Value::String(spec) => vec![spec],
            toml::Value::Array(items) => {
                let mut specs = Vec::new();
                for item in items {
                    match item {
                        toml::Value::String(spec) => specs.push(spec),
                        other => {
                            warnings.push(format!(
                                "config: keys.{name} entry `{other}` must be a string"
                            ));
                        }
                    }
                }
                specs
            }
            other => {
                warnings.push(format!(
                    "config: keys.{name}=`{other}` must be a string or array of strings"
                ));
                continue;
            }
        };
        let mut combos = Vec::new();
        for spec in specs {
            match keymap::parse_combo(&spec) {
                Ok(combo) => combos.push(combo),
                Err(err) => warnings.push(format!("config: keys.{name}: {err}")),
            }
        }
        if !combos.is_empty() {
            warnings.extend(keymap.rebind(action, combos));
        }
    }
    keymap
}

fn parse_transport(value: Option<&str>) -> Option<DaemonArg> {
    match value {
        Some("auto") => Some(DaemonArg::Auto),
        Some("uds") => Some(DaemonArg::Uds),
        Some("tcp") => Some(DaemonArg::Tcp),
        _ => None,
    }
}

fn parse_color(value: Option<&str>) -> Option<ColorMode> {
    match value {
        Some("auto") => Some(ColorMode::Auto),
        Some("never") => Some(ColorMode::Never),
        _ => None,
    }
}

fn parse_theme(value: Option<&str>) -> Option<ThemeMode> {
    match value {
        Some("auto") => Some(ThemeMode::Auto),
        Some("dark") => Some(ThemeMode::Dark),
        Some("light") => Some(ThemeMode::Light),
        _ => None,
    }
}

fn parse_color_depth(value: Option<&str>) -> Option<ColorDepthMode> {
    match value {
        Some("auto") => Some(ColorDepthMode::Auto),
        Some("ansi16") => Some(ColorDepthMode::Ansi16),
        _ => None,
    }
}

/// Resolves the config-facing `ColorDepthMode` to a concrete `ColorDepth`.
/// This is the only place in `crates/batuta` allowed to interpret
/// `COLORTERM`; the TUI crate never reads it. `Auto` becomes `TrueColor`
/// iff `COLORTERM` is `truecolor` or `24bit`, otherwise `Ansi16`.
pub fn resolve_color_depth(mode: ColorDepthMode, colorterm: Option<&str>) -> ColorDepth {
    match mode {
        ColorDepthMode::Ansi16 => ColorDepth::Ansi16,
        ColorDepthMode::Auto => match colorterm {
            Some("truecolor" | "24bit") => ColorDepth::TrueColor,
            _ => ColorDepth::Ansi16,
        },
    }
}

fn clamp<T>(value: T, min: T, max: T, name: &str, warnings: &mut Vec<String>) -> T
where
    T: Ord + Copy + fmt::Display,
{
    let clamped = value.clamp(min, max);
    if clamped != value {
        warnings.push(format!("config: {name}={value} clamped to {clamped}"));
    }
    clamped
}

fn parse_error(path: &Path, source: &str, error: toml::de::Error) -> ConfigError {
    let line = error
        .span()
        .map(|span| {
            source[..span.start]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
                + 1
        })
        .unwrap_or(1);
    ConfigError {
        path: path.to_path_buf(),
        line,
        message: error.to_string(),
    }
}

fn unknown_key_warnings(value: &toml::Value) -> Vec<String> {
    let allowed = [
        ("preset", ["agent", "loop", "provider", "model"].as_slice()),
        ("daemon", ["transport", "tcp_addr"].as_slice()),
        (
            "ui",
            [
                "color",
                "theme",
                "color_depth",
                "fps",
                "sessions_limit",
                "runs_limit",
                "notify",
            ]
            .as_slice(),
        ),
    ];
    let mut warnings = Vec::new();
    let Some(table) = value.as_table() else {
        return warnings;
    };
    for (key, value) in table {
        // `keys` is data-driven (action names, validated separately by
        // `resolve_keymap` via `keymap::action_by_name`), so its children
        // are never flagged here as unknown top-level keys.
        if key == "keys" {
            continue;
        }
        let Some((_, keys)) = allowed.iter().find(|(section, _)| section == key) else {
            warnings.push(format!("config: unknown key {key}"));
            continue;
        };
        if let Some(section) = value.as_table() {
            for child in section
                .keys()
                .filter(|child| !keys.contains(&child.as_str()))
            {
                warnings.push(format!("config: unknown key {key}.{child}"));
            }
        }
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::fs;
    use tempfile::tempdir;

    fn cli() -> Cli {
        Cli::parse_from(["batuta", "doctor"])
    }

    #[test]
    fn ut_630_missing_file_uses_dx_defaults() {
        let temp = tempdir().unwrap();
        let mut value = cli();
        value.config = Some(temp.path().join("missing.toml"));
        let settings = load_and_resolve(&value, &Env::default()).unwrap();
        assert!(!settings.loaded);
        assert_eq!(settings.preset, Preset::default());
        assert_eq!(settings.ui, UiSettings::default());
    }

    #[test]
    fn ut_631_full_file_reads_every_key() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        fs::write(&path, "[preset]\nagent='a'\nloop='l'\nprovider='p'\nmodel='m'\n[daemon]\ntransport='tcp'\ntcp_addr='x:1'\n[ui]\ncolor='never'\nfps=25\nsessions_limit=2\nruns_limit=3\n").unwrap();
        let mut value = cli();
        value.config = Some(path);
        let settings = load_and_resolve(&value, &Env::default()).unwrap();
        assert!(settings.loaded);
        assert_eq!(settings.preset.agent, "a");
        assert_eq!(settings.preset.loop_name, "l");
        assert_eq!(settings.daemon.transport, DaemonArg::Tcp);
        assert_eq!(settings.daemon.tcp_addr, "x:1");
        assert_eq!(settings.ui.color, ColorMode::Never);
        assert_eq!(settings.ui.fps, 25);
        assert_eq!(settings.ui.sessions_limit, 2);
        assert_eq!(settings.ui.runs_limit, 3);
    }

    #[test]
    fn ut_761_notify_key_resolves_and_defaults_true() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        fs::write(&path, "[ui]\nnotify=false\n").unwrap();
        let mut value = cli();
        value.config = Some(path);
        let settings = load_and_resolve(&value, &Env::default()).unwrap();
        assert!(!settings.ui.notify);

        let mut missing = cli();
        missing.config = Some(temp.path().join("missing.toml"));
        let settings = load_and_resolve(&missing, &Env::default()).unwrap();
        assert!(settings.ui.notify);
    }

    #[test]
    fn ut_770_color_depth_reads_file_warns_on_invalid_and_no_color_still_wins() {
        // File value is read.
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        fs::write(&path, "[ui]\ncolor_depth='ansi16'\n").unwrap();
        let mut value = cli();
        value.config = Some(path);
        let settings = load_and_resolve(&value, &Env::default()).unwrap();
        assert_eq!(settings.ui.color_depth, ColorDepthMode::Ansi16);

        // Missing key defaults to Auto.
        let mut missing = cli();
        missing.config = Some(temp.path().join("missing.toml"));
        let settings = load_and_resolve(&missing, &Env::default()).unwrap();
        assert_eq!(settings.ui.color_depth, ColorDepthMode::Auto);

        // Invalid value warns and falls back to Auto.
        let bad_temp = tempdir().unwrap();
        let bad_path = bad_temp.path().join("config.toml");
        fs::write(&bad_path, "[ui]\ncolor_depth='bogus'\n").unwrap();
        let mut bad = cli();
        bad.config = Some(bad_path);
        let settings = load_and_resolve(&bad, &Env::default()).unwrap();
        assert_eq!(settings.ui.color_depth, ColorDepthMode::Auto);
        assert!(
            settings
                .warnings
                .iter()
                .any(|warning| warning.contains("color_depth"))
        );

        // NO_COLOR still wins end-to-end even with a truecolor-capable
        // terminal and Auto depth.
        let env = Env {
            workspace: None,
            no_color: true,
        };
        let settings = load_and_resolve(&missing, &env).unwrap();
        assert_eq!(settings.ui.color, ColorMode::Never);
        let depth = resolve_color_depth(settings.ui.color_depth, Some("truecolor"));
        assert_eq!(depth, ColorDepth::TrueColor);
        let theme = batuta_tui::theme::Theme::with_options(
            settings.ui.color != ColorMode::Never,
            settings.ui.theme.into(),
            None,
            depth,
        );
        assert_eq!(
            theme.style(batuta_tui::theme::SemanticToken::Active).fg,
            None
        );
    }

    #[test]
    fn ut_774_keys_table_resolves_rebinds_and_warns_on_unknown_action_or_bad_combo() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        fs::write(
            &path,
            "[keys]\nquit = 'x'\nfilter = ['ctrl+f']\nbogus_action = 'z'\nrefresh = 'meh'\n",
        )
        .unwrap();
        let mut value = cli();
        value.config = Some(path);
        let settings = load_and_resolve(&value, &Env::default()).unwrap();

        // quit was rebound to `x`.
        let x = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('x'),
            crossterm::event::KeyModifiers::NONE,
        );
        assert_eq!(
            settings
                .keymap
                .action(batuta_tui::keymap::ContextGroup::Global, &x),
            Some(batuta_tui::keymap::Action::Quit)
        );
        // filter accepts an array form and was rebound to ctrl+f.
        let ctrl_f = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('f'),
            crossterm::event::KeyModifiers::CONTROL,
        );
        assert_eq!(
            settings
                .keymap
                .action(batuta_tui::keymap::ContextGroup::Lists, &ctrl_f),
            Some(batuta_tui::keymap::Action::Filter)
        );
        // unknown action name and unparsable combo warn rather than error.
        assert!(
            settings
                .warnings
                .iter()
                .any(|warning| warning.contains("unknown key action `bogus_action`"))
        );
        assert!(
            settings
                .warnings
                .iter()
                .any(|warning| warning.contains("keys.refresh"))
        );
    }

    #[test]
    fn ut_632_flags_and_environment_override_file() {
        let mut value = cli();
        value.daemon = Some(DaemonArg::Tcp);
        value.workspace = Some("flag".into());
        let settings = resolve(
            &value,
            &Env {
                workspace: Some("env".into()),
                no_color: true,
            },
            Some(ConfigFile {
                daemon: DaemonFile {
                    transport: Some("uds".into()),
                    ..DaemonFile::default()
                },
                ui: UiFile {
                    color: Some("auto".into()),
                    ..UiFile::default()
                },
                ..ConfigFile::default()
            }),
        )
        .unwrap();
        assert_eq!(settings.daemon.transport, DaemonArg::Tcp);
        assert_eq!(settings.workspace.as_deref(), Some("flag"));
        assert_eq!(settings.ui.color, ColorMode::Never);
    }

    #[test]
    fn ut_703_theme_accepts_documented_values_and_reports_invalid_value() {
        let value = cli();
        for (input, expected) in [
            ("auto", ThemeMode::Auto),
            ("dark", ThemeMode::Dark),
            ("light", ThemeMode::Light),
        ] {
            let settings = resolve(
                &value,
                &Env::default(),
                Some(ConfigFile {
                    ui: UiFile {
                        theme: Some(input.into()),
                        ..UiFile::default()
                    },
                    ..ConfigFile::default()
                }),
            )
            .unwrap();
            assert_eq!(settings.ui.theme, expected);
        }
        let error = resolve(
            &value,
            &Env::default(),
            Some(ConfigFile {
                ui: UiFile {
                    theme: Some("solarized".into()),
                    ..UiFile::default()
                },
                ..ConfigFile::default()
            }),
        )
        .unwrap_err();
        assert_eq!(error.line, 1);
        assert_eq!(error.message, "ui.theme must be auto, dark, or light");
        assert!(
            error
                .to_string()
                .contains(&error.path.display().to_string())
        );
    }

    #[test]
    fn ut_633_config_path_prefers_xdg() {
        let value = cli();
        assert_eq!(
            default_path_from(Some("/tmp/xdg".into()), Some("/tmp/home".into())),
            PathBuf::from("/tmp/xdg/batuta/config.toml")
        );
        assert_eq!(
            default_path_from(None, Some("/tmp/home".into())),
            PathBuf::from("/tmp/home/.config/batuta/config.toml")
        );
        assert!(path(&value).ends_with("batuta/config.toml"));
    }

    #[test]
    fn ut_634_and_635_errors_include_path_and_line() {
        let temp = tempdir().unwrap();
        let malformed = temp.path().join("bad.toml");
        fs::write(&malformed, "[preset]\nagent 'bad'\n").unwrap();
        let error = load(malformed.clone()).unwrap_err();
        assert_eq!(error.line, 2);
        assert!(error.to_string().contains(&malformed.display().to_string()));
        let wrong = temp.path().join("wrong.toml");
        fs::write(&wrong, "[ui]\nfps='fast'\n").unwrap();
        assert!(load(wrong).unwrap_err().to_string().contains("fps"));
    }

    #[test]
    fn ut_637_and_638_warn_and_clamp() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");
        fs::write(&path, "[ui]\ncolour='x'\nfps=200\nsessions_limit=0\n").unwrap();
        let mut value = cli();
        value.config = Some(path);
        let settings = load_and_resolve(&value, &Env::default()).unwrap();
        assert_eq!(settings.ui.fps, 60);
        assert_eq!(settings.ui.sessions_limit, 1);
        assert!(
            settings
                .warnings
                .iter()
                .any(|warning| warning.contains("ui.colour"))
        );
    }
}
