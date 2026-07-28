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
///
/// Marked `#[non_exhaustive]`: build one with [`Options::default`] and either
/// assign fields or chain the `with_*` methods.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
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
    /// Text encoding used to read each file. `None` is UTF-8.
    ///
    /// Accepts any Node `Buffer` encoding name; an unrecognised one makes the
    /// read fail, exactly as `fs.readFileSync` throws. Note `base64`, `base64url`
    /// and `hex` re-encode the bytes rather than decoding them.
    pub encoding: Option<String>,
    /// `DOTENV_KEY` for `.env.vault` decryption, overriding the environment.
    ///
    /// Comma-separated keys are tried in order, for key rotation.
    pub dotenv_key: Option<String>,
    /// What [`populate`](crate::populate) treats as "debug". `None` mirrors [`Self::debug`].
    ///
    /// The reference applies `Boolean()` to the raw option here but `parseBoolean`
    /// in `configDotenv`, so `DOTENV_CONFIG_DEBUG=false` silences config's own
    /// diagnostics while *enabling* populate's per-key lines. One `bool` cannot
    /// carry both meanings, so [`Options::from_env`] sets this separately.
    pub populate_debug: Option<bool>,
}

impl Options {
    /// Files to load. `None` is the default `./.env`; `Some(vec![])` loads nothing.
    pub fn with_path(mut self, path: Option<Vec<PathBuf>>) -> Self {
        self.path = path;
        self
    }

    /// Replace variables that are already set.
    pub fn with_overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }

    /// Print per-key decisions to stdout.
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Suppress the summary line.
    pub fn with_quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    /// Text encoding used to read each file. `None` is UTF-8.
    pub fn with_encoding(mut self, encoding: Option<String>) -> Self {
        self.encoding = encoding;
        self
    }

    /// `DOTENV_KEY` for `.env.vault` decryption.
    pub fn with_dotenv_key(mut self, key: Option<String>) -> Self {
        self.dotenv_key = key;
        self
    }

    /// The options built from `DOTENV_CONFIG_*`, ported from the reference's
    /// `lib/env-options.js`.
    ///
    /// For the whole `dotenv/config` preload -- which also reads `argv` -- use
    /// [`Options::for_preload`].
    ///
    /// Note `DOTENV_CONFIG_OVERRIDE` and `DOTENV_CONFIG_DEBUG`: the reference
    /// copies the raw strings into the options object, and `populate` applies
    /// `Boolean()` to them rather than `parseBoolean`. So *any* non-empty value --
    /// including `"false"`, `"0"` and `"off"` -- turns overriding on and enables
    /// populate's per-key lines. Both are reproduced here.
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
            // `Boolean(rawString)`, not `parseBoolean` -- see the note above.
            options.populate_debug = Some(!value.is_empty());
        }
        if let Some(value) = non_empty("DOTENV_CONFIG_ENCODING") {
            options.encoding = Some(value);
        }
        if let Some(value) = non_empty("DOTENV_CONFIG_DOTENV_KEY") {
            options.dotenv_key = Some(value);
        }
        if let Ok(value) = std::env::var("DOTENV_CONFIG_OVERRIDE") {
            options.overwrite = !value.is_empty();
        }

        options
    }

    /// `dotenv_config_<name>=<value>` arguments, ported from `lib/cli-options.js`.
    ///
    /// Recognised names are `encoding`, `path`, `quiet`, `debug`, `override` and
    /// `DOTENV_KEY`. Note the reference forces `quiet` on unless
    /// `dotenv_config_quiet=` appears, so the preload is silent by default even
    /// though `config()` is not.
    pub fn from_cli<I, S>(args: I) -> Options
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut options = Options::default();
        let mut saw_quiet = false;

        for argument in args {
            let Some((name, value)) = argument.as_ref().split_once('=') else {
                continue;
            };
            let Some(name) = name.strip_prefix("dotenv_config_") else {
                continue;
            };
            // The reference's regex requires a non-empty value.
            if value.is_empty() {
                continue;
            }

            match name {
                "encoding" => options.encoding = Some(value.to_string()),
                "path" => options.path = Some(vec![PathBuf::from(value)]),
                "debug" => {
                    options.debug = parse_boolean(value);
                    options.populate_debug = Some(true);
                }
                // `Boolean(rawString)`: any non-empty value is truthy.
                "override" => options.overwrite = true,
                "quiet" => {
                    options.quiet = parse_boolean(value);
                    saw_quiet = true;
                }
                "DOTENV_KEY" => options.dotenv_key = Some(value.to_string()),
                _ => {}
            }
        }

        if !saw_quiet {
            options.quiet = true;
        }
        options
    }

    /// The options the `dotenv/config` preload builds:
    /// `Object.assign({}, env-options, cli-options(argv))`.
    ///
    /// Command-line arguments win over environment variables, and `quiet`
    /// defaults on.
    pub fn for_preload<I, S>(args: I) -> Options
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let env = Options::from_env();
        let mut merged = Options::from_cli(args);

        // Anything the command line did not set falls back to the environment.
        if merged.path.is_none() {
            merged.path = env.path;
        }
        if merged.encoding.is_none() {
            merged.encoding = env.encoding;
        }
        if merged.dotenv_key.is_none() {
            merged.dotenv_key = env.dotenv_key;
        }
        merged.overwrite |= env.overwrite;
        merged.debug |= env.debug;
        merged.populate_debug = merged.populate_debug.or(env.populate_debug);
        merged
    }
}

/// Where `config` reads existing variables from and writes results to -- the
/// reference's `processEnv` option, which defaults to `process.env`.
///
/// The reference reads `DOTENV_CONFIG_DEBUG`/`QUIET` from this object too, not
/// from the real environment, so a supplied map fully replaces it.
pub(crate) enum Target<'a> {
    Process,
    Map(&'a mut EnvMap),
}

impl Target<'_> {
    fn get(&self, key: &str) -> Option<String> {
        match self {
            Target::Process => std::env::var(key).ok(),
            Target::Map(map) => map.get(key).cloned(),
        }
    }

    /// `Object.prototype.hasOwnProperty.call(processEnv, key)`.
    fn contains(&self, key: &str) -> bool {
        match self {
            // `var_os`, not `var`: a variable holding non-UTF-8 bytes still
            // exists and must not be silently replaced.
            Target::Process => std::env::var_os(key).is_some(),
            Target::Map(map) => map.contains_key(key),
        }
    }

    /// # Safety
    ///
    /// For [`Target::Process`], the caller must ensure no other thread touches
    /// the environment concurrently. [`Target::Map`] is always safe.
    unsafe fn set(&mut self, key: &str, value: &str) {
        match self {
            Target::Process => {
                // The reference tolerates a NUL byte here -- libuv truncates the
                // exported value at it -- while `set_var` would panic, taking the
                // process down over a malformed line. A plain object keeps the
                // whole value, so this only applies to the real environment.
                let value = match value.find('\0') {
                    Some(at) => &value[..at],
                    None => value,
                };
                // SAFETY: guaranteed by this function's own contract.
                unsafe { std::env::set_var(key, value) };
            }
            Target::Map(map) => {
                map.insert(key.to_string(), value.to_string());
            }
        }
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
/// reading the environment concurrently -- including from C code, and including
/// reads this crate never sees -- is undefined behaviour. The caller must ensure
/// no other thread touches the environment for the duration of the call, which
/// in practice means calling it early in `main`, before spawning any threads.
///
/// [`parse`](crate::parse) and [`populate`](crate::populate) are safe and touch
/// no global state; use them if you want to apply a `.env` yourself.
pub unsafe fn config() -> ConfigResult {
    // SAFETY: forwarded to our own caller by `config` being `unsafe`.
    unsafe { config_options(&Options::default()) }
}

/// `config(options)` against an arbitrary target.
///
/// # Safety
///
/// See [`config`] when `target` is `Target::Process`.
unsafe fn config_in(target: &mut Target<'_>, options: &Options) -> ConfigResult {
    if crate::vault::dotenv_key(options).is_none() {
        // SAFETY: forwarded to our own caller.
        return unsafe { config_dotenv_in(target, options) };
    }

    let Some(path) = crate::vault::vault_path(options) else {
        warn("you set DOTENV_KEY but you are missing a .env.vault file at null");
        // SAFETY: forwarded to our own caller.
        return unsafe { config_dotenv_in(target, options) };
    };

    // SAFETY: forwarded to our own caller.
    unsafe { config_vault_in(target, options, path) }
}

/// [`config`] with explicit options. Ports the reference's `config(options)`.
///
/// # Safety
///
/// See [`config`].
pub unsafe fn config_options(options: &Options) -> ConfigResult {
    // SAFETY: forwarded to our own caller.
    unsafe { config_in(&mut Target::Process, options) }
}

/// Load `./.env` into `target` instead of the process environment.
///
/// This is the reference's `processEnv` option. Because nothing global is
/// written, it is safe -- no `unsafe` and no thread-safety obligation.
///
/// ```
/// use dotenv_compat::{EnvMap, Options};
///
/// let mut env = EnvMap::new();
/// let result = dotenv_compat::config_into(&mut env, &Options::default());
/// # let _ = result;
/// ```
pub fn config_into(target: &mut EnvMap, options: &Options) -> ConfigResult {
    // SAFETY: a `Map` target never touches the process environment.
    unsafe { config_in(&mut Target::Map(target), options) }
}

/// [`config_into`] without the `DOTENV_KEY` vault handling -- the reference's
/// `configDotenv({ processEnv })`.
pub fn config_with_into(target: &mut EnvMap, options: &Options) -> ConfigResult {
    // SAFETY: a `Map` target never touches the process environment.
    unsafe { config_dotenv_in(&mut Target::Map(target), options) }
}

/// `_configVault(options)`.
///
/// # Safety
///
/// See [`config`].
unsafe fn config_vault_in(
    target: &mut Target<'_>,
    options: &Options,
    path: PathBuf,
) -> ConfigResult {
    let debug = target_flag(target, "DOTENV_CONFIG_DEBUG", options.debug);
    let quiet = target_flag(target, "DOTENV_CONFIG_QUIET", options.quiet);
    if debug || !quiet {
        log("loading env from encrypted .env.vault");
    }

    // SAFETY: forwarded to our own caller.
    match unsafe { crate::vault::parse_vault(target, options, path) } {
        Ok(parsed) => {
            // SAFETY: forwarded to our own caller.
            unsafe { populate_target(target, &parsed, options) };
            ConfigResult {
                parsed,
                error: None,
            }
        }
        // The reference throws here; Rust has no exceptions, so it surfaces on
        // `error` with an empty `parsed`.
        Err(error) => ConfigResult {
            parsed: EnvMap::new(),
            error: Some(error),
        },
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
/// See [`config`] -- the caller must ensure no other thread reads or writes the
/// environment for the duration of the call.
pub unsafe fn config_with(options: &Options) -> ConfigResult {
    // SAFETY: forwarded to our own caller.
    unsafe { config_dotenv_in(&mut Target::Process, options) }
}

/// `configDotenv(options)` against an arbitrary target.
///
/// # Safety
///
/// See [`config`] when `target` is `Target::Process`.
pub(crate) unsafe fn config_dotenv_in(target: &mut Target<'_>, options: &Options) -> ConfigResult {
    // `parseBoolean(processEnv.DOTENV_CONFIG_DEBUG || options.debug)`
    let mut debug = target_flag(target, "DOTENV_CONFIG_DEBUG", options.debug);
    let mut quiet = target_flag(target, "DOTENV_CONFIG_QUIET", options.quiet);

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
        match read_decoded(path, options.encoding.as_deref()) {
            Ok(text) => {
                let parsed = crate::parse(text.as_bytes());
                crate::populate(&mut parsed_all, &parsed, options);
            }
            Err(error) => {
                if debug {
                    debug_log(&format!("failed to load {} {error}", path.display()));
                }
                last_error = Some(error);
            }
        }
    }

    // SAFETY: forwarded to our own caller by `config_with` being `unsafe`.
    let populated = unsafe { populate_target(target, &parsed_all, options) };

    // Re-read, so a `.env` that sets DOTENV_CONFIG_QUIET silences its own summary.
    debug = target_flag(target, "DOTENV_CONFIG_DEBUG", debug);
    quiet = target_flag(target, "DOTENV_CONFIG_QUIET", quiet);

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

/// # Safety
///
/// The caller must ensure no other thread touches the environment concurrently.
unsafe fn populate_target(target: &mut Target<'_>, parsed: &EnvMap, options: &Options) -> EnvMap {
    // Snapshot first: `set` needs a mutable borrow of the same target.
    let existing: Vec<String> = parsed
        .keys()
        .filter(|key| target.contains(key))
        .cloned()
        .collect();

    let mut writer = |key: &str, value: &str| {
        // SAFETY: forwarded to our own caller.
        unsafe { target.set(key, value) };
    };

    populate_with(
        parsed,
        options,
        |key| existing.iter().any(|k| k == key),
        &mut writer,
    )
}

/// `fs.readFileSync(path, { encoding })`.
///
/// An unknown encoding name fails before the file is opened, matching the
/// `TypeError` the reference throws.
fn read_decoded(path: &Path, encoding: Option<&str>) -> Result<String, Error> {
    let encoding = match encoding {
        None => crate::encoding::Encoding::Utf8,
        Some(name) => crate::encoding::Encoding::from_name(name)
            .ok_or_else(|| Error::InvalidEncoding(name.to_string()))?,
    };

    let bytes = std::fs::read(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;

    Ok(encoding.decode(&bytes))
}

/// `parseBoolean(processEnv[name] || fallback)`.
///
/// An unset or empty variable leaves `fallback` in charge; any other value is run
/// through `parse_boolean`, so `DOTENV_CONFIG_QUIET=false` forces it off even when
/// the caller asked for `quiet: true`.
fn target_flag(target: &Target<'_>, name: &str, fallback: bool) -> bool {
    match target.get(name).filter(|value| !value.is_empty()) {
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

pub(crate) fn cwd() -> PathBuf {
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
/// Note an *empty* `HOME` is used as-is, matching libuv; only an unset one falls
/// through to the password database.
fn home_dir() -> Option<PathBuf> {
    let name = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    if let Some(value) = std::env::var_os(name) {
        return Some(PathBuf::from(value));
    }
    passwd_home()
}

/// `getpwuid_r(getuid()).pw_dir`, the fallback libuv uses when `HOME` is unset.
#[cfg(unix)]
fn passwd_home() -> Option<PathBuf> {
    use std::ffi::{CStr, OsString};
    use std::os::unix::ffi::OsStringExt;

    // SAFETY: getuid is always safe; it cannot fail and touches no memory.
    let uid = unsafe { libc::getuid() };

    let mut buffer = vec![0u8; 2048];
    loop {
        let mut passwd: libc::passwd = unsafe { std::mem::zeroed() };
        let mut result: *mut libc::passwd = std::ptr::null_mut();

        // SAFETY: `passwd` and `buffer` are live and correctly sized for the
        // duration of the call, and `result` receives either null or a pointer
        // into `passwd`.
        let code = unsafe {
            libc::getpwuid_r(
                uid,
                &mut passwd,
                buffer.as_mut_ptr() as *mut libc::c_char,
                buffer.len(),
                &mut result,
            )
        };

        if code == libc::ERANGE && buffer.len() < 1 << 20 {
            buffer.resize(buffer.len() * 2, 0);
            continue;
        }
        if code != 0 || result.is_null() || passwd.pw_dir.is_null() {
            return None;
        }

        // SAFETY: pw_dir points into `buffer`, which outlives this read.
        let dir = unsafe { CStr::from_ptr(passwd.pw_dir) };
        let dir = OsString::from_vec(dir.to_bytes().to_vec());
        return match dir.is_empty() {
            true => None,
            false => Some(PathBuf::from(dir)),
        };
    }
}

/// `GetUserProfileDirectoryW`, the fallback libuv uses on Windows when
/// `USERPROFILE` is unset.
#[cfg(windows)]
fn passwd_home() -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::TOKEN_QUERY;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows_sys::Win32::UI::Shell::GetUserProfileDirectoryW;

    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: `token` is a valid out-pointer. `GetCurrentProcess` returns a
    // pseudo-handle that must not be closed, which is why only `token` is.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return None;
    }

    // A null buffer asks for the required length, in UTF-16 units.
    let mut len: u32 = 0;
    // SAFETY: the documented way to query the size; failure is expected here.
    unsafe { GetUserProfileDirectoryW(token, std::ptr::null_mut(), &mut len) };

    let mut buffer = vec![0u16; len as usize];
    // SAFETY: `buffer` holds exactly the `len` units just requested.
    let ok = unsafe { GetUserProfileDirectoryW(token, buffer.as_mut_ptr(), &mut len) };
    // SAFETY: `token` came from `OpenProcessToken` and is not used again.
    unsafe { CloseHandle(token) };

    if ok == 0 {
        return None;
    }

    // Trust the NUL terminator rather than the returned length, as libuv does.
    let end = buffer
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(buffer.len());
    let dir = OsString::from_wide(&buffer[..end]);
    (!dir.is_empty()).then(|| PathBuf::from(dir))
}

/// Neither Unix nor Windows: no password database to consult.
#[cfg(not(any(unix, windows)))]
fn passwd_home() -> Option<PathBuf> {
    None
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
