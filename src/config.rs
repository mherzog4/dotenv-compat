//! Port of dotenv's `configDotenv()`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::populate::populate_with;

/// Settings for [`config_with`].
///
/// Two documented differences from the JavaScript original:
///
/// * There is no `encoding` option. Files are read as UTF-8, with invalid
///   sequences replaced, so `DOTENV_CONFIG_ENCODING` is ignored.
/// * `DOTENV_CONFIG_*` variables are only consulted by [`Options::from_env`]
///   (which [`config`] uses). An `Options` you build yourself is taken verbatim.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Files to load, in order. Empty means `./.env`. A leading `~` is expanded
    /// to the home directory.
    pub path: Vec<PathBuf>,
    /// Replace variables that are already set. `override` in the JavaScript API.
    pub overwrite: bool,
    /// Print per-key decisions to stdout.
    pub debug: bool,
    /// Suppress the summary line printed to stderr after loading.
    pub quiet: bool,
}

impl Options {
    /// Defaults taken from `DOTENV_CONFIG_PATH`, `DOTENV_CONFIG_QUIET`,
    /// `DOTENV_CONFIG_DEBUG` and `DOTENV_CONFIG_OVERRIDE`.
    pub fn from_env() -> Options {
        let mut options = Options::default();
        if let Some(path) = std::env::var_os("DOTENV_CONFIG_PATH") {
            options.path = vec![PathBuf::from(path)];
        }
        if let Ok(value) = std::env::var("DOTENV_CONFIG_QUIET") {
            options.quiet = parse_boolean(&value);
        }
        if let Ok(value) = std::env::var("DOTENV_CONFIG_DEBUG") {
            options.debug = parse_boolean(&value);
        }
        if let Ok(value) = std::env::var("DOTENV_CONFIG_OVERRIDE") {
            options.overwrite = parse_boolean(&value);
        }
        options
    }
}

/// What [`config`] loaded.
#[derive(Debug)]
pub struct ConfigResult {
    /// Every key parsed across all files, before the process environment was consulted.
    pub parsed: HashMap<String, String>,
    /// The last file that failed to load, if any. Missing files are not fatal.
    pub error: Option<Error>,
}

/// Load `./.env` into the process environment, with defaults from `DOTENV_CONFIG_*`.
///
/// # Safety
///
/// This calls [`std::env::set_var`], which is not thread-safe: another thread
/// reading the environment concurrently (including from C code) is undefined
/// behaviour. Call this early in `main`, before spawning any threads.
pub fn config() -> ConfigResult {
    config_with(&Options::from_env())
}

/// Load the configured files into the process environment.
///
/// # Safety
///
/// See [`config`] -- must be called before any other thread touches the environment.
pub fn config_with(options: &Options) -> ConfigResult {
    let paths: Vec<PathBuf> = if options.path.is_empty() {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        vec![cwd.join(".env")]
    } else {
        options.path.iter().map(|p| resolve_home(p)).collect()
    };

    // Files are merged into one map first, so `overwrite` decides which file wins
    // among themselves before the process environment is touched at all.
    let mut parsed_all: HashMap<String, String> = HashMap::new();
    let mut last_error = None;

    for path in &paths {
        match std::fs::read(path) {
            Ok(bytes) => {
                let parsed = crate::parse(&bytes);
                crate::populate(&mut parsed_all, &parsed, options);
            }
            Err(source) => {
                if options.debug {
                    debug(&format!("failed to load {} {source}", path.display()));
                }
                last_error = Some(Error::Io {
                    path: path.clone(),
                    source,
                });
            }
        }
    }

    let populated = populate_process_env(&parsed_all, options);

    if options.debug || !options.quiet {
        let shown: Vec<String> = paths.iter().map(|p| relative_to_cwd(p)).collect();
        log(&format!(
            "injected env ({}) from {}",
            populated.len(),
            shown.join(",")
        ));
    }

    ConfigResult {
        parsed: parsed_all,
        error: last_error,
    }
}

fn populate_process_env(
    parsed: &HashMap<String, String>,
    options: &Options,
) -> HashMap<String, String> {
    let mut set = |key: &str, value: &str| {
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

/// `!['false', '0', 'no', 'off', ''].includes(value.toLowerCase())`
fn parse_boolean(value: &str) -> bool {
    !matches!(
        value.to_lowercase().as_str(),
        "false" | "0" | "no" | "off" | ""
    )
}

/// Expand a leading `~` to the home directory, matching dotenv's `_resolveHome`.
fn resolve_home(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    match (text.strip_prefix('~'), home_dir()) {
        // `join` would discard the base if the remainder looked absolute.
        (Some(rest), Some(home)) => home.join(rest.trim_start_matches(['/', '\\'])),
        _ => path.to_path_buf(),
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Shorten a path for the summary line. Unlike Node's `path.relative` this never
/// produces `../` segments; unrelated paths are printed in full.
fn relative_to_cwd(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(&cwd).ok().map(|p| p.display().to_string()))
        .unwrap_or_else(|| path.display().to_string())
}

pub(crate) fn debug(message: &str) {
    println!("┆ {message}");
}

fn log(message: &str) {
    eprintln!("◇ {message}");
}
