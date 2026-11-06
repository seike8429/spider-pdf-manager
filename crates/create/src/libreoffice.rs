//! The LibreOffice engine: an installed `soffice` / `libreoffice` binary driven as an
//! optional external process (`soffice --headless --convert-to pdf --outdir <dir> <file>`),
//! never linked, never bundled, never downloaded. The whole process-spawning part is compiled
//! out for wasm32, where nothing may spawn; there the conversion is simply unavailable.
//!
//! Security posture (hard rules, mirroring `ocr::tesseract`): the program path comes from the
//! PATH or a user-configured location, never from document data; the input file is a private
//! temp copy whose name is fixed (`input.docx` / `.xlsx` / `.pptx` — the extension is
//! allow-listed, never taken from the document); the run has a timeout and an output size cap;
//! each run gets its own fresh LibreOffice user profile (`-env:UserInstallation`, single
//! dash: the double-dash form makes LibreOffice 24 print its usage and exit 1), so a
//! second conversion never contends with a locked profile; and every failure comes back
//! through [`LibreOfficeError`] with its cause named.
//!
//! Scope (v1.0): OOXML only (`.docx`, `.xlsx`, `.pptx`). Legacy `.doc` / `.xls` / `.ppt`
//! are not converted. LibreOffice is installed-only: when it is absent the conversion fails
//! with an actionable error naming where to get it — it is never silently skipped and never
//! auto-downloaded.

use std::path::{Path, PathBuf};
use std::time::Duration;

// Native only: the web build never spawns a process, so nothing there needs the extension.
#[cfg(not(target_arch = "wasm32"))]
use super::office_extension;

/// How long one conversion may run before the process is killed.
const TIMEOUT: Duration = Duration::from_secs(120);
/// How much of the program's stderr is read back (its end): enough for LibreOffice's error
/// text, small enough that error text stays a message, not a memory DoS.
const MAX_STDERR: u64 = 256 * 1024;
/// The largest PDF a conversion may produce.
const MAX_PDF_BYTES: u64 = 512 * 1024 * 1024;

/// What to tell a user whose computer has no LibreOffice: the program, where to get it, and
/// that the conversion runs locally (nothing is downloaded or uploaded).
pub const NOT_INSTALLED: &str = "LibreOffice is not installed — install LibreOffice from https://www.libreoffice.org/download/ and try again. Office files are converted on this computer with the installed LibreOffice program; nothing is downloaded or sent anywhere";

/// What to tell a user on a build where no program can ever run (the web build).
#[cfg(target_arch = "wasm32")]
pub const NOT_AVAILABLE_WEB: &str =
    "Office conversion is not available in the web version — open the file in the desktop app, where an installed LibreOffice converts it";

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum LibreOfficeError {
    #[error("{0}")]
    NotAvailable(String),
    #[error("LibreOffice did not finish converting the file within {0} seconds")]
    Timeout(u64),
    #[error("LibreOffice produced more than {0} bytes of output")]
    OutputTooLarge(u64),
    #[error("LibreOffice failed: {0}")]
    Process(String),
    #[error("LibreOffice finished but produced no PDF file{0}")]
    MissingOutput(String),
    #[error("this file cannot be converted with LibreOffice: {0}")]
    Invalid(String),
}

/// Whether `program` is a usable program location: an absolute path to an existing,
/// executable FILE (on Unix the executable bits must be set; elsewhere being a file is what
/// the OS requires). A directory or a missing file is simply unavailable — never a panic.
fn program_is_executable(program: &Path) -> bool {
    if !program.is_absolute() || !program.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(program).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// The file names an installed LibreOffice binary goes by, in the order they are tried
/// (`libreoffice` first, then `soffice`).
fn program_file_names() -> &'static [&'static str] {
    #[cfg(windows)]
    {
        &["soffice.exe", "libreoffice.exe"]
    }
    #[cfg(not(windows))]
    {
        &["libreoffice", "soffice"]
    }
}

/// The user-configured program location, when valid: `$PDFCRAFT_LIBREOFFICE` (a user
/// preference, never taken from document data). An invalid value — relative, missing, a
/// directory, not executable — means unavailable: an explicit setting is never silently
/// replaced by a PATH search.
pub fn configured_path() -> Option<PathBuf> {
    let value = std::env::var_os("PDFCRAFT_LIBREOFFICE")?;
    let program = PathBuf::from(value);
    program_is_executable(&program).then_some(program)
}

/// The program [`LibreOfficeCli::find`] would run: a set `configured` location (the value of
/// `PDFCRAFT_LIBREOFFICE`) decides alone, so an invalid one yields `None` and `search` (the
/// PATH) is not even consulted; only when nothing is configured does `search` decide.
/// (Native only: the web build never spawns a process.)
#[cfg(not(target_arch = "wasm32"))]
fn locate(configured: Option<std::ffi::OsString>, search: impl FnOnce() -> Option<PathBuf>) -> Option<PathBuf> {
    match configured {
        Some(value) => Some(PathBuf::from(value)).filter(|program| program_is_executable(program)),
        None => search(),
    }
}

/// The first LibreOffice on the PATH that is an existing executable file. Entries that are
/// not absolute are refused: a relative PATH entry resolves against the caller's working
/// directory, which is neither stable nor a safe place to find a program.
pub fn path_search() -> Option<PathBuf> {
    path_search_in(std::env::split_paths(&std::env::var_os("PATH")?))
}

/// [`path_search`] over an explicit directory list (so tests can pass one).
fn path_search_in(dirs: impl Iterator<Item = PathBuf>) -> Option<PathBuf> {
    for dir in dirs.filter(|d| d.is_absolute()) {
        for name in program_file_names() {
            let candidate = dir.join(name);
            if program_is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

/// A `file:///` URL for `path` (what `--env:UserInstallation` takes): backslashes become
/// slashes, and every byte that is not a plain URL character is percent-encoded.
/// (Native only: the web build never spawns a process.)
#[cfg(not(target_arch = "wasm32"))]
fn file_url(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let mut url = String::from("file:///");
    for b in text.trim_start_matches('/').as_bytes() {
        let c = *b as char;
        if c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.' | '~' | ':') {
            url.push(c);
        } else {
            url.push_str(&format!("%{b:02X}"));
        }
    }
    url
}

/// Make a just-created file owner-only, where the OS supports it (Unix). Windows relies on the
/// user's private %TEMP% ACLs; the fresh directory keeps other sessions' files out of sight.
/// (Native only: the web build never spawns a process.)
#[cfg(not(target_arch = "wasm32"))]
fn set_private(f: &mut std::fs::File) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = f.set_permissions(std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = f;
}

/// A fresh directory under the system temp dir, deleted (with its contents) when dropped.
/// On Unix it is created 0700, so the input copy inside is private.
/// (Native only: the web build never spawns a process.)
#[cfg(not(target_arch = "wasm32"))]
struct TempDir {
    path: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl TempDir {
    fn create() -> Result<TempDir, LibreOfficeError> {
        // Conversions can run at once, and on a coarse clock two runs can read the same
        // nanosecond. One counter value per attempt keeps the names of one process apart
        // without trusting the clock.
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        // Created owner-only in one step on Unix (no window in which the directory exists with
        // the default, wider mode); `create` refuses a path that already exists.
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        #[cfg(not(unix))]
        let _ = &mut builder;
        // A leftover directory of an earlier process with the same pid can still take a name;
        // that one collision is retried with the next counter value, anything else is fatal.
        for _ in 0..4 {
            let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("pdfcraft-libreoffice-{}-{nanos}-{seq}", std::process::id()));
            match builder.create(&path) {
                Ok(()) => return Ok(TempDir { path }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(LibreOfficeError::Process(format!("creating the private temp directory failed: {e}"))),
            }
        }
        Err(LibreOfficeError::Process("creating the private temp directory failed: every candidate name already exists".into()))
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// The LibreOffice program. Build one (via [`LibreOfficeCli::find`] for the installed copy,
/// or [`LibreOfficeCli::new`] for a user-configured path) and convert with it.
pub struct LibreOfficeCli {
    program: PathBuf,
    /// The timeout and the output cap are fields so tests can shorten them. (On the web build
    /// no process ever runs, so they are never read there.)
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    timeout: Duration,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    max_output: u64,
}

impl LibreOfficeCli {
    /// The program at `path` (a user preference; never taken from document data).
    pub fn new(program: PathBuf) -> LibreOfficeCli {
        LibreOfficeCli { program, timeout: TIMEOUT, max_output: MAX_PDF_BYTES }
    }

    /// The program this converter runs.
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// The program at a user-configured `path`, if it is usable there: only an absolute path
    /// to an existing, executable file is accepted (a directory, a missing file or a relative
    /// path is unavailable, never a panic). No probe run is made — LibreOffice takes a second
    /// or so to start, and a broken binary surfaces as a conversion error naming the cause.
    pub fn at(program: PathBuf) -> Option<LibreOfficeCli> {
        program_is_executable(&program).then(|| LibreOfficeCli::new(program))
    }

    /// The program this machine offers, if any: the user-configured location
    /// ([`configured_path`] / `$PDFCRAFT_LIBREOFFICE`) when valid, else the first absolute
    /// PATH entry that holds an executable `libreoffice` or `soffice`.
    ///
    /// # The PATH-lookup risk
    ///
    /// Resolving through the PATH trusts every directory on it: a binary planted earlier on
    /// the PATH runs as a subprocess with this application's privileges — it sees the input
    /// file and anything the process can reach. A user who handles untrusted documents should
    /// pin the exact program with `PDFCRAFT_LIBREOFFICE`, which is validated (absolute,
    /// existing, executable file) before any process starts.
    ///
    /// A `PDFCRAFT_LIBREOFFICE` that is set but invalid fails closed: it is never replaced by
    /// the PATH search.
    pub fn find() -> Option<LibreOfficeCli> {
        #[cfg(target_arch = "wasm32")]
        {
            None
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            LibreOfficeCli::at(locate(std::env::var_os("PDFCRAFT_LIBREOFFICE"), path_search)?)
        }
    }

    /// Convert `bytes` named `name` (`.docx`, `.xlsx` or `.pptx`) to PDF bytes.
    pub fn convert_bytes(&self, name: &str, bytes: &[u8]) -> Result<Vec<u8>, LibreOfficeError> {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (name, bytes);
            return Err(LibreOfficeError::NotAvailable(NOT_AVAILABLE_WEB.into()));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let ext = office_extension(name)
                .ok_or_else(|| LibreOfficeError::Invalid("only .docx, .xlsx and .pptx files are converted with LibreOffice".into()))?;
            if !bytes.starts_with(b"PK\x03\x04") {
                return Err(LibreOfficeError::Invalid("the file is not an Office document (it does not start with the OOXML zip header)".into()));
            }
            let dir = TempDir::create()?;
            // The input copy's name is fixed: only the allow-listed extension is kept, so a
            // hostile file name can neither escape the temp dir nor confuse the converter
            // about the format. LibreOffice names its output after the input (`input.pdf`).
            let input = dir.path.join(format!("input.{ext}"));
            {
                use std::io::Write;
                let mut f = std::fs::File::create(&input).map_err(|e| LibreOfficeError::Process(format!("creating the input copy failed: {e}")))?;
                set_private(&mut f);
                f.write_all(bytes).map_err(|e| LibreOfficeError::Process(format!("writing the input copy failed: {e}")))?;
            }
            self.convert_file(&input, &dir)
        }
    }

    /// Convert the Office file at `input` (inside private `dir`) to PDF bytes.
    #[cfg(not(target_arch = "wasm32"))]
    fn convert_file(&self, input: &Path, dir: &TempDir) -> Result<Vec<u8>, LibreOfficeError> {
        let outdir = dir.path.join("out");
        std::fs::create_dir(&outdir).map_err(|e| LibreOfficeError::Process(format!("creating the output directory failed: {e}")))?;
        // A fresh profile per run: without it, a second conversion refuses to start against
        // the locked profile of a LibreOffice that is already running.
        let profile = dir.path.join("profile");
        let install = file_url(&profile);
        let stderr = self.run(&[
            "--headless",
            &format!("-env:UserInstallation={install}"),
            "--convert-to",
            "pdf",
            "--outdir",
            &outdir.to_string_lossy(),
            &input.to_string_lossy(),
        ])?;
        let stem = input.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let output = outdir.join(format!("{stem}.pdf"));
        if !output.is_file() {
            let note = stderr.trim();
            return Err(LibreOfficeError::MissingOutput(if note.is_empty() { String::new() } else { format!(": {note}") }));
        }
        let len = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
        if len > self.max_output {
            return Err(LibreOfficeError::OutputTooLarge(self.max_output));
        }
        let pdf = std::fs::read(&output).map_err(|e| LibreOfficeError::Process(format!("reading the converted PDF failed: {e}")))?;
        if !pdf.starts_with(b"%PDF-") {
            return Err(LibreOfficeError::Process("LibreOffice did not produce a PDF file".into()));
        }
        Ok(pdf)
    }

    /// Run the program with fixed argv, its stdout and stderr in private temp files, under
    /// `timeout` seconds. Returns the stderr tail (empty when the program stayed quiet), for
    /// the error when the expected output never appears.
    #[cfg(not(target_arch = "wasm32"))]
    fn run(&self, args: &[&str]) -> Result<String, LibreOfficeError> {
        let dir = TempDir::create()?;
        let out_path = dir.path.join("out.txt");
        let err_path = dir.path.join("err.txt");
        let out_file = std::fs::File::create(&out_path).map_err(|e| LibreOfficeError::Process(format!("creating the output file failed: {e}")))?;
        // stderr goes to a file, not a pipe: an undrained pipe fills (around 64 KiB) and then
        // blocks the child mid-write, which a watcher can only read as a timeout. A file never
        // blocks; it is read back as a short tail after the run.
        let err_file = std::fs::File::create(&err_path).map_err(|e| LibreOfficeError::Process(format!("creating the error file failed: {e}")))?;
        // Known limit: only the direct child is killed at the timeout or on an early cap. A
        // wrapper script's children (grandchildren of this process) may survive the kill; the
        // cap and the timeout still bound what this run reads and how long it waits.
        let mut child = std::process::Command::new(&self.program)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(out_file)
            .stderr(err_file)
            .current_dir(&dir.path)
            // A minimal environment: nothing from the document or the caller's shell beyond
            // what an installed LibreOffice needs — the PATH (bare-name lookup and DLL
            // loading), the Windows system root, a home directory (fontconfig and the
            // profile machinery), and a temp dir it may write to.
            .env_clear()
            .envs(std::env::vars().filter(|(k, _)| {
                k.eq_ignore_ascii_case("PATH")
                    || k.eq_ignore_ascii_case("SystemRoot")
                    || k.eq_ignore_ascii_case("HOME")
                    || k.eq_ignore_ascii_case("TMPDIR")
            }))
            .env("TMP", &dir.path)
            .env("TEMP", &dir.path)
            .spawn()
            .map_err(|e| LibreOfficeError::NotAvailable(format!("{} could not start: {e}", self.program.display())))?;
        let started = std::time::Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => {
                    // The output caps are enforced while the child runs: a flood is killed as
                    // soon as a file passes its cap, not after it has finished writing.
                    if std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0) > self.max_output
                        || std::fs::metadata(&err_path).map(|m| m.len()).unwrap_or(0) > MAX_STDERR
                    {
                        let _ = child.kill();
                        let _ = child.wait();
                        break Err(LibreOfficeError::OutputTooLarge(self.max_output));
                    }
                    if started.elapsed() >= self.timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        break Err(LibreOfficeError::Timeout(self.timeout.as_secs()));
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(LibreOfficeError::Process(format!("waiting for {} failed: {e}", self.program.display())));
                }
            }
        };
        let status = status?;
        // Whatever the exit, the stderr tail names the cause (capped: error text stays a
        // message, not a memory DoS).
        let stderr = std::fs::File::open(&err_path)
            .and_then(|mut f| {
                use std::io::{Read, Seek, SeekFrom};
                let len = f.metadata()?.len().min(MAX_STDERR);
                f.seek(SeekFrom::End(-(len as i64)))?;
                let mut buf = Vec::with_capacity(len as usize);
                f.take(len).read_to_end(&mut buf)?;
                Ok::<_, std::io::Error>(buf)
            })
            .unwrap_or_default();
        let stderr = String::from_utf8_lossy(&stderr).into_owned();
        // The message names only the end of it.
        let tail = stderr.char_indices().rev().nth(2047).map_or(0, |(i, _)| i);
        let note = stderr.get(tail..).unwrap_or_default().trim().to_string();
        if !status.success() {
            return Err(LibreOfficeError::Process(format!(
                "{} exited with {}: {}",
                self.program.display(),
                status.code().map(|c| c.to_string()).unwrap_or_else(|| "a signal".into()),
                if note.is_empty() { "no error message" } else { &note }
            )));
        }
        Ok(note)
    }
}

/// Convert `bytes` named `name` (`.docx`, `.xlsx` or `.pptx`) to PDF bytes with the installed
/// LibreOffice, or fail with [`LibreOfficeError::NotAvailable`] naming where to get it.
pub fn convert_office_to_pdf(name: &str, bytes: &[u8]) -> Result<Vec<u8>, LibreOfficeError> {
    let Some(cli) = LibreOfficeCli::find() else {
        #[cfg(target_arch = "wasm32")]
        return Err(LibreOfficeError::NotAvailable(NOT_AVAILABLE_WEB.into()));
        #[cfg(not(target_arch = "wasm32"))]
        return Err(LibreOfficeError::NotAvailable(NOT_INSTALLED.into()));
    };
    cli.convert_bytes(name, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn file_url_encodes_a_path() {
        assert_eq!(file_url(Path::new("/tmp/lo profile/x")), "file:///tmp/lo%20profile/x");
        assert_eq!(file_url(Path::new("/tmp/plain")), "file:///tmp/plain");
    }

    /// The private temp directory is created owner-only (Unix) and removed on drop.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn temp_dir_is_private_and_removed() {
        let dir = TempDir::create().unwrap();
        let path = dir.path.clone();
        assert!(path.is_dir());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o700);
        }
        drop(dir);
        assert!(!path.exists());
    }

    /// A set `PDFCRAFT_LIBREOFFICE` decides alone: when it is invalid, [`LibreOfficeCli::find`]
    /// does not fall back to a PATH copy (which would run a program the user did not name); the
    /// PATH search only runs when nothing is configured.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn an_invalid_configured_program_fails_closed() {
        let Ok(real) = std::env::current_exe() else { return };
        let searched = std::cell::Cell::new(false);
        let on_path = || {
            searched.set(true);
            Some(real.clone())
        };
        assert_eq!(locate(None, on_path), Some(real.clone()), "nothing configured: the PATH search decides");
        assert!(searched.get());
        searched.set(false);
        assert_eq!(locate(Some(real.clone().into_os_string()), on_path), Some(real.clone()), "a valid location is used as is");
        assert!(!searched.get(), "and the PATH is not searched");
        #[cfg(unix)]
        use std::path::Path as P;
        #[cfg(unix)]
        let dir = real.parent().map(P::to_path_buf).unwrap_or_default().into_os_string();
        #[cfg(not(unix))]
        let dir = std::ffi::OsString::from("");
        for bad in [std::ffi::OsString::from("relative/soffice"), std::ffi::OsString::from(""), dir, "/no/such/program-0123456789".into()] {
            assert_eq!(locate(Some(bad.clone()), on_path), None, "{bad:?} is invalid, so there is no program");
            assert!(!searched.get(), "{bad:?}: an invalid location never falls back to the PATH");
        }
    }

    /// The PATH search resolves through absolute entries only and skips relative ones: a
    /// relative PATH entry would resolve against the caller's working directory. Either
    /// binary name counts, `libreoffice` first.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn path_search_skips_relative_entries() {
        let dir = std::env::temp_dir().join(format!("pdfcraft-lo-path-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let program = dir.join(program_file_names()[0]);
        std::fs::write(&program, b"").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755));
        }
        let found = path_search_in([PathBuf::from("relative/dir"), PathBuf::from("definitely/missing"), dir.clone()].into_iter());
        assert_eq!(found.as_deref(), Some(program.as_path()), "the absolute entry with the program wins, relative entries are skipped");
        let none = path_search_in([PathBuf::from("relative/dir")].into_iter());
        assert_eq!(none, None, "only relative entries: nothing is found");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A configured program location is only usable as an absolute path to an existing,
    /// executable file: a relative path, a missing file and a directory are unavailable (and
    /// [`LibreOfficeCli::at`] refuses them without spawning anything).
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_configured_program_must_be_an_absolute_executable_file() {
        assert!(!program_is_executable(Path::new(".")), "relative");
        assert!(!program_is_executable(Path::new("definitely/missing/soffice")), "missing");
        let temp = std::env::temp_dir();
        assert!(!program_is_executable(&temp), "a directory is not a program");
        let file = temp.join(format!("pdfcraft-lo-not-a-program-{}", std::process::id()));
        std::fs::write(&file, b"").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644));
            assert!(!program_is_executable(&file), "a file without executable bits is not a program");
        }
        #[cfg(not(unix))]
        assert!(program_is_executable(&file), "an existing file is usable on this platform");
        let _ = std::fs::remove_file(&file);
        // `at` validates before any process starts, so these are None and not panics.
        assert!(LibreOfficeCli::at(PathBuf::from(".")).is_none());
        assert!(LibreOfficeCli::at(temp).is_none());
        assert!(LibreOfficeCli::at(PathBuf::from("definitely/missing/soffice")).is_none());
    }

    /// Without LibreOffice the error names the missing program and where to get it.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn missing_libreoffice_names_the_program_and_where_to_get_it() {
        let err = convert_office_to_pdf("report.docx", b"PK\x03\x04").unwrap_err();
        // On a machine with LibreOffice this converts the (truncated) input instead; the
        // assertion only pins the message when the binary is absent.
        if matches!(err, LibreOfficeError::NotAvailable(_)) {
            assert!(err.to_string().contains("LibreOffice is not installed"), "{err}");
            assert!(err.to_string().contains("libreoffice.org/download"), "{err}");
        }
    }

    /// Bytes that are not an OOXML zip are refused before any process starts.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn non_office_bytes_are_refused_without_spawning() {
        let cli = LibreOfficeCli::new(PathBuf::from("/no/such/soffice"));
        let err = cli.convert_bytes("report.docx", b"hello").unwrap_err();
        assert!(matches!(err, LibreOfficeError::Invalid(_)), "{err}");
        let err = cli.convert_bytes("report.txt", b"PK\x03\x04").unwrap_err();
        assert!(matches!(err, LibreOfficeError::Invalid(_)), "a .txt is not an Office file: {err}");
    }

    /// A fake `soffice` (Unix only): it writes a minimal PDF next to the input, as
    /// `--convert-to pdf --outdir <dir>` would, and the conversion comes back as PDF bytes.
    /// The script also records its argv, so the test can check the profile isolation flag.
    #[cfg(unix)]
    #[test]
    fn a_fake_soffice_converts_and_gets_its_own_profile() {
        let dir = std::env::temp_dir().join(format!("pdfcraft-lo-fake-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("soffice");
        std::fs::write(
            &script,
            "#!/bin/sh\n\
             echo \"$@\" > \"$(dirname \"$0\")/argv.txt\"\n\
             outdir=\"\"; input=\"\"\n\
             prev=\"\"\n\
             for a in \"$@\"; do\n\
             if [ \"$prev\" = \"--outdir\" ]; then outdir=\"$a\"; fi\n\
             prev=\"$a\"; input=\"$a\";\n\
             done\n\
             printf '%%PDF-1.7\\n' > \"$outdir/input.pdf\"\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755));
        let cli = LibreOfficeCli::new(script);
        let pdf = cli.convert_bytes("report.docx", b"PK\x03\x04junk").unwrap();
        assert!(pdf.starts_with(b"%PDF-"), "{pdf:?}");
        let argv = std::fs::read_to_string(dir.join("argv.txt")).unwrap();
        assert!(argv.contains("--headless"), "headless: {argv}");
        assert!(argv.contains("--convert-to pdf"), "convert to pdf: {argv}");
        assert!(argv.contains("-env:UserInstallation=file:///"), "its own profile per run: {argv}");
        assert!(argv.contains("input.docx"), "the input copy keeps its extension: {argv}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A program that exits successfully without writing a PDF is a missing-output error,
    /// not a silent empty result (Unix only).
    #[cfg(unix)]
    #[test]
    fn a_silent_program_is_a_missing_output_error() {
        let dir = std::env::temp_dir().join(format!("pdfcraft-lo-silent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("soffice");
        std::fs::write(&script, "#!/bin/sh\necho 'no pdf for you' >&2\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755));
        let cli = LibreOfficeCli::new(script);
        let err = cli.convert_bytes("report.docx", b"PK\x03\x04junk").unwrap_err();
        assert!(matches!(err, LibreOfficeError::MissingOutput(_)), "{err}");
        assert!(err.to_string().contains("no pdf for you"), "the stderr tail names the cause: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A hung program is killed at the timeout and reported (Unix only).
    #[cfg(unix)]
    #[test]
    fn a_hung_program_is_killed_at_the_timeout() {
        let dir = std::env::temp_dir().join(format!("pdfcraft-lo-hang-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("soffice");
        std::fs::write(&script, "#!/bin/sh\nsleep 30\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755));
        let mut cli = LibreOfficeCli::new(script);
        cli.timeout = Duration::from_millis(500);
        let started = std::time::Instant::now();
        let err = cli.convert_bytes("report.docx", b"PK\x03\x04junk").unwrap_err();
        assert!(matches!(err, LibreOfficeError::Timeout(_)), "{err}");
        assert!(started.elapsed() < Duration::from_secs(10), "killed, not waited out");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The real binary, when it is installed: a minimal .docx converts end to end (skipped
    /// where LibreOffice is missing — CI without it must not fail).
    #[test]
    fn converts_a_minimal_docx_with_the_installed_binary() {
        let Some(cli) = LibreOfficeCli::find() else {
            eprintln!("skipped: LibreOffice is not installed");
            return;
        };
        let pdf = cli.convert_bytes("report.docx", &minimal_docx_bytes()).unwrap();
        assert!(pdf.starts_with(b"%PDF-"), "LibreOffice produced a PDF");
    }

    /// A minimal .docx as stored zip entries (no compression, so no deflate is needed):
    /// `[Content_Types].xml` + `_rels/.rels` + `word/document.xml` with one paragraph.
    /// The `_rels/.rels` part is required: without it LibreOffice refuses the file
    /// ("source file could not be loaded").
    fn minimal_docx_bytes() -> Vec<u8> {
        let mut out = Vec::new();
        let mut directory: Vec<u8> = Vec::new();
        let mut count: u16 = 0;
        for (name, data) in [
            (
                "[Content_Types].xml",
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/></Types>",
            ),
            (
                "_rels/.rels",
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/></Relationships>",
            ),
            (
                "word/document.xml",
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body><w:p><w:r><w:t>Hello</w:t></w:r></w:p></w:body></w:document>",
            ),
        ] {
            let data = data.as_bytes();
            let offset = out.len() as u32;
            let name_len = name.len() as u16;
            let data_len = data.len() as u32;
            out.extend_from_slice(b"PK\x03\x04");
            out.extend_from_slice(&20u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&data_len.to_le_bytes());
            out.extend_from_slice(&data_len.to_le_bytes());
            out.extend_from_slice(&name_len.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(data);
            directory.extend_from_slice(b"PK\x01\x02");
            directory.extend_from_slice(&20u16.to_le_bytes());
            directory.extend_from_slice(&20u16.to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&0u32.to_le_bytes());
            directory.extend_from_slice(&data_len.to_le_bytes());
            directory.extend_from_slice(&data_len.to_le_bytes());
            directory.extend_from_slice(&name_len.to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&0u16.to_le_bytes());
            directory.extend_from_slice(&0u32.to_le_bytes());
            directory.extend_from_slice(&offset.to_le_bytes());
            directory.extend_from_slice(name.as_bytes());
            count += 1;
        }
        let dir_offset = out.len() as u32;
        out.extend_from_slice(&directory);
        let dir_len = directory.len() as u32;
        out.extend_from_slice(b"PK\x05\x06");
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&dir_len.to_le_bytes());
        out.extend_from_slice(&dir_offset.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }
}
