//! Port of dotenv's `configDotenv()` and `config()`.
//!
//! Ported against the `lib/main.js` that `dotenv@17.4.2` actually ships on npm.
//! (The `master` branch on GitHub is a later refactor with the vault code removed
//! and different logging; it is not what this crate targets.)

use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::map::EnvMap;
use crate::populate::populate_with;

/// The tip suffix the reference appends to its summary line.
const TIPS: [&str; 8] = [
    "◈ encrypted .env [www.dotenvx.com]",
    "◈ secrets for agents [www.dotenvx.com]",
    "⌁ auth for agents [www.vestauth.com]",
    "⌘ custom filepath { path: '/custom/path/.env' }",
    "⌘ enable debugging { debug: true }",
    "⌘ override existing { override: true }",
    "⌘ suppress logs { quiet: true }",
    "⌘ multiple files { path: ['.env.local', '.env'] }",
];

/// Settings for [`config_with`].
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Files to load, in order.
    ///
    /// `None` means the default `./.env`. `Some(vec![])` means load nothing --
    /// the same distinction JavaScript draws between `undefined` and `[]`.
    /// A leading `~` is expanded to the home directory.
    pub path: Option<Vec<PathBuf>>,
    /// Replace variables that are already set. `override` in the JavaScript API.
    pub overwrite: bool,
    /// Print per-key decisions to stdout. `DOTENV_CONFIG_DEBUG` takes precedence.
    pub debug: bool,
    /// Suppress the summary line. `DOTENV_CONFIG_QUIET` takes precedence.
    pub quiet: bool,
}

impl Options {
    /// The options `dotenv/config` builds from `DOTENV_CONFIG_*`, ported from
    /// the reference's `lib/env-options.js`.
    ///
    /// Note `DOTENV_CONFIG_OVERRIDE`: the reference copies the raw string into
    /// `options.override` and `populate` then applies `Boolean()` to it, not
    /// `parseBoolean`. So *any* non-empty value -- including `"false"`, `"0"`
    /// and `"off"` -- turns overriding ON. That is reproduced here.
    pub fn from_env() -> Options {
        let mut options = Options::default();

        // An empty string is falsy in JavaScript, so it leaves the default in place.
        if let Some(path) = non_empty("DOTENV_CONFIG_PATH") {
            options.path = Some(vec![PathBuf::from(path)]);
        }
        if let Ok(value) = std::env::var("DOTENV_CONFIG_QUIET") {
            options.quiet = parse_boolean(&value);
        }
        if let Ok(value) = std::env::var("DOTENV_CONFIG_DEBUG") {
            options.debug = parse_boolean(&value);
        }
        if let Ok(value) = std::env::var("DOTENV_CONFIG_OVERRIDE") {
            options.overwrite = !value.is_empty();
        }

        options
    }
}

/// What [`config`] loaded.
#[derive(Debug)]
pub struct ConfigResult {
    /// Every key parsed across all files, before the process environment was consulted.
    pub parsed: EnvMap,
    /// The last file that failed to load, if any. Missing files are not fatal.
    pub error: Option<Error>,
}

/// Load `./.env` into the process environment.
///
/// Ports the reference's `config()`: when `DOTENV_KEY` is set it looks for a
/// `.env.vault`, warning and falling back to the plain `.env` when there is none.
///
/// # Safety
///
/// This calls [`std::env::set_var`], which is not thread-safe: another thread
/// reading the environment concurrently (including from C code) is undefined
/// behaviour. Call this early in `main`, before spawning any threads.
pub fn config() -> ConfigResult {
    let options = Options::default();

    if dotenv_key().is_none() {
        return config_with(&options);
    }

    match vault_path(&options) {
        None => {
            // The reference interpolates the null it just computed.
            warn("you set DOTENV_KEY but you are missing a .env.vault file at null");
            config_with(&options)
        }
        Some(path) => {
            // Falling back to a plain `.env` would load a different set of secrets,
            // so refuse rather than quietly diverge.
            warn(&format!(
                "found {} but dotenv-compat cannot decrypt .env.vault files; nothing was loaded",
                path.display()
            ));
            ConfigResult {
                parsed: EnvMap::new(),
                error: Some(Error::VaultUnsupported { path }),
            }
        }
    }
}

/// `_dotenvKey()` -- the `DOTENV_KEY` environment variable, if non-empty.
fn dotenv_key() -> Option<String> {
    non_empty("DOTENV_KEY")
}

/// `_vaultPath(options)`.
fn vault_path(options: &Options) -> Option<PathBuf> {
    let candidate = match &options.path {
        Some(paths) if !paths.is_empty() => {
            // The reference keeps the *last* existing entry, not the first.
            let mut found = None;
            for path in paths {
                if path.exists() {
                    found = Some(with_vault_suffix(path));
                }
            }
            found?
        }
        _ => cwd().join(".env.vault"),
    };

    candidate.exists().then_some(candidate)
}

fn with_vault_suffix(path: &Path) -> PathBuf {
    match path.to_string_lossy() {
        text if text.ends_with(".vault") => path.to_path_buf(),
        text => PathBuf::from(format!("{text}.vault")),
    }
}

/// Load the configured files into the process environment.
///
/// `DOTENV_CONFIG_DEBUG` and `DOTENV_CONFIG_QUIET` are read from the environment
/// on every call and take precedence over `options`, matching the reference. The
/// other `DOTENV_CONFIG_*` variables apply only through [`Options::from_env`].
///
/// # Safety
///
/// See [`config`] -- must be called before any other thread touches the environment.
pub fn config_with(options: &Options) -> ConfigResult {
    // `parseBoolean(processEnv.DOTENV_CONFIG_DEBUG || options.debug)`
    let mut debug = env_flag("DOTENV_CONFIG_DEBUG", options.debug);
    let mut quiet = env_flag("DOTENV_CONFIG_QUIET", options.quiet);

    if debug {
        // There is no `encoding` option here, so the reference's else-branch always
        // applies.
        debug_log("no encoding is specified (UTF-8 is used by default)");
    }

    let paths: Vec<PathBuf> = match &options.path {
        // An explicitly empty list loads nothing; only an absent list defaults.
        Some(given) => given.iter().map(|p| resolve_home(p)).collect(),
        None => vec![cwd().join(".env")],
    };

    // Files are merged into one map first, so `overwrite` decides which file wins
    // among themselves before the process environment is touched at all.
    let mut parsed_all = EnvMap::new();
    let mut last_error = None;

    for path in &paths {
        match std::fs::read(path) {
            Ok(bytes) => {
                let parsed = crate::parse(&bytes);
                crate::populate(&mut parsed_all, &parsed, options);
            }
            Err(source) => {
                if debug {
                    debug_log(&format!(
                        "failed to load {} {}",
                        path.display(),
                        crate::error::node_message(path, &source)
                    ));
                }
                last_error = Some(Error::Io {
                    path: path.clone(),
                    source,
                });
            }
        }
    }

    let populated = populate_process_env(&parsed_all, options);

    // Re-read, so a `.env` that sets DOTENV_CONFIG_QUIET silences its own summary.
    debug = env_flag("DOTENV_CONFIG_DEBUG", debug);
    quiet = env_flag("DOTENV_CONFIG_QUIET", quiet);

    if debug || !quiet {
        let shown: Vec<String> = paths.iter().map(|p| relative_to_cwd(p)).collect();
        log(&format!(
            "injected env ({}) from {} {}",
            populated.len(),
            shown.join(","),
            dim(&format!("// tip: {}", random_tip()))
        ));
    }

    ConfigResult {
        parsed: parsed_all,
        error: last_error,
    }
}

fn populate_process_env(parsed: &EnvMap, options: &Options) -> EnvMap {
    let mut set = |key: &str, value: &str| {
        // The reference tolerates a NUL byte here (libuv truncates the exported
        // value at it); `set_var` would panic, taking the whole process down for
        // what is at worst a malformed line. Truncating keeps the crash away and
        // lands on the same exported value.
        let value = match value.find('\0') {
            Some(at) => &value[..at],
            None => value,
        };
        // SAFETY: documented on `config` -- the caller must not have other threads
        // running. There is no thread-safe way to write the process environment.
        unsafe { std::env::set_var(key, value) };
    };
    // `var_os`, not `vars()`: a variable holding non-UTF-8 bytes still exists and
    // must not be silently replaced.
    populate_with(
        parsed,
        options,
        |key| std::env::var_os(key).is_some(),
        &mut set,
    )
}

/// `parseBoolean(processEnv[name] || fallback)`.
///
/// An unset or empty variable leaves `fallback` in charge; any other value is run
/// through `parse_boolean`, so `DOTENV_CONFIG_QUIET=false` forces it off even when
/// the caller asked for `quiet: true`.
fn env_flag(name: &str, fallback: bool) -> bool {
    match non_empty(name) {
        Some(value) => parse_boolean(&value),
        None => fallback,
    }
}

fn non_empty(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// `!['false', '0', 'no', 'off', ''].includes(value.toLowerCase())`
fn parse_boolean(value: &str) -> bool {
    !matches!(
        value.to_lowercase().as_str(),
        "false" | "0" | "no" | "off" | ""
    )
}

fn cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// `envPath[0] === '~' ? path.join(os.homedir(), envPath.slice(1)) : envPath`
///
/// `path.join` normalises `..` lexically, so `~/../x` is the textual parent of the
/// home directory even when the home directory is a symlink.
fn resolve_home(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    match (text.strip_prefix('~'), home_dir()) {
        (Some(rest), Some(home)) => {
            PathBuf::from(crate::nodepath::join(&home.to_string_lossy(), rest))
        }
        _ => path.to_path_buf(),
    }
}

/// `os.homedir()`.
///
/// Known gap: on Unix, Node falls back to `getpwuid()` when `HOME` is unset. That
/// needs libc, so with no `HOME` we leave the `~` unexpanded instead.
fn home_dir() -> Option<PathBuf> {
    let name = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(name).map(PathBuf::from)
}

/// `path.relative(process.cwd(), filePath)`.
fn relative_to_cwd(path: &Path) -> String {
    crate::nodepath::relative(&cwd().to_string_lossy(), &path.to_string_lossy())
}

/// `Math.floor(Math.random() * TIPS.length)`.
///
/// `RandomState` is seeded per instance by the OS, which is entropy enough to pick
/// one of eight strings.
fn random_tip() -> &'static str {
    use std::hash::{BuildHasher, Hasher};
    let seed = std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish();
    TIPS[(seed % TIPS.len() as u64) as usize]
}

fn dim(text: &str) -> String {
    if std::io::stdout().is_terminal() {
        format!("\x1b[2m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

pub(crate) fn debug_log(message: &str) {
    println!("┆ {message}");
}

fn log(message: &str) {
    println!("◇ {message}");
}

fn warn(message: &str) {
    eprintln!("⚠ {message}");
}
