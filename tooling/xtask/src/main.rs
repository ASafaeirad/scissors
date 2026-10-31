use std::{
    collections::BTreeSet,
    env,
    ffi::OsStr,
    fmt, fs, io,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
};

use sha2::{Digest, Sha256};
use syn::{
    Expr, ImplItemFn, ItemFn, Lit, UnOp,
    visit::{self, Visit},
};

const FFMPEG_VERSION: &str = "8.1.2";
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

type Result<T> = std::result::Result<T, XtaskError>;

fn main() -> ExitCode {
    match run_xtask() {
        Ok(()) | Err(XtaskError::Help) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run_xtask() -> Result<()> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_help();
        return Ok(());
    };

    match command.as_str() {
        "build" => build_frame_app(args.collect()),
        "ci" => ci(),
        "run" => run_frame_app(args.collect()),
        "setup-ffmpeg" => {
            let args = args.collect::<Vec<_>>();
            setup_ffmpeg(&args)
        }
        "-h" | "--help" | "help" => {
            print_help();
            Ok(())
        }
        _ => Err(XtaskError::Usage(format!("unknown command `{command}`"))),
    }
}

fn print_help() {
    println!(
        "\
Usage: cargo xtask <command>

Commands:
  run               Run the native Frame app
  build             Build frame-app
  setup-ffmpeg      Download FFmpeg and FFprobe runtime binaries
  ci                Run local formatting, tests, lints, and UI color checks
"
    );
}

fn run_frame_app(args: Vec<String>) -> Result<()> {
    let mut cargo_args = vec![
        "run".to_string(),
        "--manifest-path".to_string(),
        "frame-app/Cargo.toml".to_string(),
    ];
    cargo_args.extend(args);
    run_command_path("cargo", &cargo_args)
}

fn build_frame_app(args: Vec<String>) -> Result<()> {
    let mut cargo_args = vec![
        "build".to_string(),
        "--manifest-path".to_string(),
        "frame-app/Cargo.toml".to_string(),
    ];
    cargo_args.extend(args);
    run_command_path("cargo", &cargo_args)
}

fn setup_ffmpeg(args: &[String]) -> Result<()> {
    let options = SetupFfmpegOptions::parse(args)?;
    let target = ffmpeg_target_for(
        options
            .platform
            .as_deref()
            .unwrap_or_else(|| host_platform()),
        options.arch.as_deref().unwrap_or(host_arch()),
    )?;
    let binary_dir = repo_root()?.join("frame-app/resources/binaries");

    fs::create_dir_all(&binary_dir)?;
    println!(
        "Detected {}. Preparing FFmpeg {FFMPEG_VERSION} binaries...",
        target.label()
    );

    match target {
        FfmpegTarget::Individual { binaries, .. } => {
            for entry in binaries {
                process_ffmpeg_entry(entry, &binary_dir, options.force)?;
            }
        }
        FfmpegTarget::SharedArchive {
            archive, entries, ..
        } => {
            process_ffmpeg_shared_archive(archive, entries, &binary_dir, options.force)?;
        }
    }

    println!("All binaries are ready in frame-app/resources/binaries.");
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct SetupFfmpegOptions {
    force: bool,
    platform: Option<String>,
    arch: Option<String>,
}

impl SetupFfmpegOptions {
    fn parse(args: &[String]) -> Result<Self> {
        let mut force = false;
        let mut platform = None;
        let mut arch = None;
        let mut index = 0;

        while index < args.len() {
            match args[index].as_str() {
                "--force" => {
                    force = true;
                    index += 1;
                }
                "--platform" => {
                    let Some(value) = args.get(index + 1) else {
                        return Err(XtaskError::Usage(
                            "missing value for --platform".to_string(),
                        ));
                    };
                    platform = Some(value.clone());
                    index += 2;
                }
                "--arch" => {
                    let Some(value) = args.get(index + 1) else {
                        return Err(XtaskError::Usage("missing value for --arch".to_string()));
                    };
                    arch = Some(value.clone());
                    index += 2;
                }
                "-h" | "--help" => {
                    println!(
                        "\
Usage: cargo xtask setup-ffmpeg [options]

Options:
  --force              Re-download binaries even when they already exist
  --platform <name>    Override platform: darwin, linux, or win32
  --arch <name>        Override architecture: x64, x86_64, arm64, or aarch64
"
                    );
                    return Err(XtaskError::Help);
                }
                other => {
                    return Err(XtaskError::Usage(format!(
                        "unknown setup-ffmpeg option `{other}`"
                    )));
                }
            }
        }

        Ok(Self {
            force,
            platform,
            arch,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FfmpegBinaryEntry {
    id: &'static str,
    archive: Option<FfmpegArchive>,
    expected_names: &'static [&'static str],
    destination_name: &'static str,
    sha256: &'static str,
    make_executable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FfmpegArchive {
    url: &'static str,
    sha256: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FfmpegTarget {
    Individual {
        label: &'static str,
        binaries: &'static [FfmpegBinaryEntry],
    },
    SharedArchive {
        label: &'static str,
        archive: FfmpegArchive,
        entries: &'static [FfmpegBinaryEntry],
    },
}

impl FfmpegTarget {
    const fn label(&self) -> &'static str {
        match self {
            Self::Individual { label, .. } | Self::SharedArchive { label, .. } => label,
        }
    }
}

const MACOS_X86_64_BINARIES: &[FfmpegBinaryEntry] = &[
    FfmpegBinaryEntry {
        id: "ffmpeg",
        archive: Some(FfmpegArchive {
            url: "https://ffmpeg.martin-riedl.de/download/macos/amd64/1783018342_8.1.2/ffmpeg.zip",
            sha256: "a52ef43883f44c219766d4b3bdde4e635b35465d0b704c01c3a0566b59775df9",
        }),
        expected_names: &["ffmpeg"],
        destination_name: "ffmpeg-x86_64-apple-darwin",
        sha256: "1ca59dda73668c59898a0b305afd8a88817a989187f222ec62d64e775d614d23",
        make_executable: true,
    },
    FfmpegBinaryEntry {
        id: "ffprobe",
        archive: Some(FfmpegArchive {
            url: "https://ffmpeg.martin-riedl.de/download/macos/amd64/1783018342_8.1.2/ffprobe.zip",
            sha256: "5408ca588c8c72b0dde3afe676d0a7acf25ef97e55ae6eba5c7bede1cda42695",
        }),
        expected_names: &["ffprobe"],
        destination_name: "ffprobe-x86_64-apple-darwin",
        sha256: "bdb6aff0f1f414382effd97040f7862dc85e67996ac296cb4288beed0e06498f",
        make_executable: true,
    },
];

const MACOS_AARCH64_BINARIES: &[FfmpegBinaryEntry] = &[
    FfmpegBinaryEntry {
        id: "ffmpeg",
        archive: Some(FfmpegArchive {
            url: "https://ffmpeg.martin-riedl.de/download/macos/arm64/1783011502_8.1.2/ffmpeg.zip",
            sha256: "ef1aa60006c7b77ce170c1608c08d8e4ba1c30c5746f2ac986ded932d0ac2c3c",
        }),
        expected_names: &["ffmpeg"],
        destination_name: "ffmpeg-aarch64-apple-darwin",
        sha256: "eaf91238e104dd0e262bc6510e25061855cc99a6955a721b0ac99660d58c473d",
        make_executable: true,
    },
    FfmpegBinaryEntry {
        id: "ffprobe",
        archive: Some(FfmpegArchive {
            url: "https://ffmpeg.martin-riedl.de/download/macos/arm64/1783011502_8.1.2/ffprobe.zip",
            sha256: "c39787f4af7a3932502d2d48db6f6feaaa836b48a73ef78c32cc3285df61dfaf",
        }),
        expected_names: &["ffprobe"],
        destination_name: "ffprobe-aarch64-apple-darwin",
        sha256: "ed9dc5871914b466b96b402c9ec0ba68ce4f836e72faa464b1b4e279835bd4a6",
        make_executable: true,
    },
];

const LINUX_X86_64_BINARIES: &[FfmpegBinaryEntry] = &[
    FfmpegBinaryEntry {
        id: "ffmpeg",
        archive: Some(FfmpegArchive {
            url: "https://ffmpeg.martin-riedl.de/download/linux/amd64/1783011670_8.1.2/ffmpeg.zip",
            sha256: "56452c0bfc4ee0325cd615d62f46ba8264f62eed34f727c2224c6c84fa7b8719",
        }),
        expected_names: &["ffmpeg"],
        destination_name: "ffmpeg-x86_64-unknown-linux-gnu",
        sha256: "bea0dfb96f7223b1be497cf11ccda9ddd9a39103b948b342bb6db1c60a56be12",
        make_executable: true,
    },
    FfmpegBinaryEntry {
        id: "ffprobe",
        archive: Some(FfmpegArchive {
            url: "https://ffmpeg.martin-riedl.de/download/linux/amd64/1783011670_8.1.2/ffprobe.zip",
            sha256: "c6f2d36e98f9a4445fad0b0be539f4c4faf13fd502116bf131becd53f56cd390",
        }),
        expected_names: &["ffprobe"],
        destination_name: "ffprobe-x86_64-unknown-linux-gnu",
        sha256: "f0a9c3c87d45fe323ae893fe9820150a46f5af9fc5f75066712097f160befac5",
        make_executable: true,
    },
];

const LINUX_AARCH64_BINARIES: &[FfmpegBinaryEntry] = &[
    FfmpegBinaryEntry {
        id: "ffmpeg",
        archive: Some(FfmpegArchive {
            url: "https://ffmpeg.martin-riedl.de/download/linux/arm64/1783010599_8.1.2/ffmpeg.zip",
            sha256: "ab9e16864b6bf4ae7e13bbdbdc29621be11a5c547c57af8d4250e9fa2f5e6461",
        }),
        expected_names: &["ffmpeg"],
        destination_name: "ffmpeg-aarch64-unknown-linux-gnu",
        sha256: "93a3684e7467d33881f8fa39e3b8408248d4f95fb2e9f6b18383edcdbd70f163",
        make_executable: true,
    },
    FfmpegBinaryEntry {
        id: "ffprobe",
        archive: Some(FfmpegArchive {
            url: "https://ffmpeg.martin-riedl.de/download/linux/arm64/1783010599_8.1.2/ffprobe.zip",
            sha256: "fb78317b81cdeb614533be59e489019b754afd199670666af28f0e9574be395b",
        }),
        expected_names: &["ffprobe"],
        destination_name: "ffprobe-aarch64-unknown-linux-gnu",
        sha256: "7a4103c64cd78c7c634a5610ea3ae5dd3a97b3714cc831407c668decf6a34c6d",
        make_executable: true,
    },
];

// BtbN retains month-end snapshots for two years; ordinary daily builds expire after 14 days.
const WINDOWS_X86_64_ARCHIVE: FfmpegArchive = FfmpegArchive {
    url: "https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-06-30-13-34/ffmpeg-n8.1.2-21-gce3c09c101-win64-gpl-8.1.zip",
    sha256: "682361e32c9631caec09e5d9f09077101c9ed90c14e275f62014fefa6d397990",
};

const WINDOWS_X86_64_BINARIES: &[FfmpegBinaryEntry] = &[
    FfmpegBinaryEntry {
        id: "ffmpeg",
        archive: None,
        expected_names: &["ffmpeg.exe"],
        destination_name: "ffmpeg-x86_64-pc-windows-msvc.exe",
        sha256: "c47e9e15e76897778915ba16e36c8002b0a3f2f9e7c0a71aa1d41459ac1d02d1",
        make_executable: false,
    },
    FfmpegBinaryEntry {
        id: "ffprobe",
        archive: None,
        expected_names: &["ffprobe.exe"],
        destination_name: "ffprobe-x86_64-pc-windows-msvc.exe",
        sha256: "2864c7a71b820b07d3a9666bb4389c8af4bb9449876b07a75b3b7f15adbdafaa",
        make_executable: false,
    },
];

fn ffmpeg_target_for(platform: &str, arch: &str) -> Result<FfmpegTarget> {
    match (platform, arch) {
        ("darwin", "x64" | "x86_64") => {
            Ok(martin_ffmpeg_target("macOS (Intel)", MACOS_X86_64_BINARIES))
        }
        ("darwin", "arm64" | "aarch64") => Ok(martin_ffmpeg_target(
            "macOS (Apple Silicon)",
            MACOS_AARCH64_BINARIES,
        )),
        ("linux", "x64" | "x86_64" | "amd64") => {
            Ok(martin_ffmpeg_target("Linux x86_64", LINUX_X86_64_BINARIES))
        }
        ("linux", "arm64" | "aarch64") => {
            Ok(martin_ffmpeg_target("Linux ARM64", LINUX_AARCH64_BINARIES))
        }
        ("win32" | "windows", "x64" | "x86_64") => Ok(windows_ffmpeg_target()),
        _ => Err(XtaskError::Usage(format!(
            "unsupported platform or architecture: {platform}/{arch}"
        ))),
    }
}

const fn martin_ffmpeg_target(
    label: &'static str,
    binaries: &'static [FfmpegBinaryEntry],
) -> FfmpegTarget {
    FfmpegTarget::Individual { label, binaries }
}

const fn windows_ffmpeg_target() -> FfmpegTarget {
    FfmpegTarget::SharedArchive {
        label: "Windows x86_64",
        archive: WINDOWS_X86_64_ARCHIVE,
        entries: WINDOWS_X86_64_BINARIES,
    }
}

fn process_ffmpeg_entry(entry: &FfmpegBinaryEntry, binary_dir: &Path, force: bool) -> Result<()> {
    let destination = binary_dir.join(entry.destination_name);
    if !ffmpeg_binary_needs_download(entry, &destination, force)? {
        return Ok(());
    }

    let Some(archive) = entry.archive else {
        return Err(XtaskError::Usage(format!(
            "missing download URL for {}",
            entry.id
        )));
    };

    println!("Downloading {} from {}...", entry.id, archive.url);
    let archive_bytes = download_file(archive.url)?;
    verify_bytes_sha256(&archive_bytes, archive.sha256, archive.url)?;
    extract_expected_file(&archive_bytes, entry, &destination)
}

fn process_ffmpeg_shared_archive(
    archive: FfmpegArchive,
    entries: &[FfmpegBinaryEntry],
    binary_dir: &Path,
    force: bool,
) -> Result<()> {
    let destinations = entries
        .iter()
        .map(|entry| binary_dir.join(entry.destination_name))
        .collect::<Vec<_>>();
    let needs_download = entries
        .iter()
        .zip(&destinations)
        .map(|(entry, destination)| ffmpeg_binary_needs_download(entry, destination, force))
        .collect::<Result<Vec<_>>>()?;

    if !needs_download.iter().any(|needs_download| *needs_download) {
        return Ok(());
    }

    println!("Downloading Windows bundle from {}...", archive.url);
    let archive_bytes = download_file(archive.url)?;
    verify_bytes_sha256(&archive_bytes, archive.sha256, archive.url)?;

    for ((entry, destination), needs_download) in
        entries.iter().zip(&destinations).zip(needs_download)
    {
        if !needs_download {
            continue;
        }
        extract_expected_file(&archive_bytes, entry, destination)?;
    }

    Ok(())
}

fn ffmpeg_binary_needs_download(
    entry: &FfmpegBinaryEntry,
    destination: &Path,
    force: bool,
) -> Result<bool> {
    if force || !destination.is_file() {
        return Ok(true);
    }

    let actual = file_sha256_hex(destination)?;
    if actual == entry.sha256 {
        if entry.make_executable {
            make_file_executable(destination)?;
        }
        println!("Verified cached {} (SHA-256).", entry.destination_name);
        Ok(false)
    } else {
        println!(
            "Cached {} has an unexpected SHA-256; downloading it again.",
            entry.destination_name
        );
        Ok(true)
    }
}

fn download_file(url: &str) -> Result<Vec<u8>> {
    let response = ureq::get(url)
        .call()
        .map_err(|source| XtaskError::Download {
            url: url.to_string(),
            source: Box::new(source),
        })?;
    let mut bytes = Vec::new();
    response.into_body().into_reader().read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn verify_bytes_sha256(bytes: &[u8], expected: &str, subject: &str) -> Result<()> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    verify_sha256(&actual, expected, subject)
}

fn file_sha256_hex(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher)?;
    let actual = format!("{:x}", hasher.finalize());
    Ok(actual)
}

fn verify_file_sha256(path: &Path, expected: &str) -> Result<()> {
    verify_sha256(
        &file_sha256_hex(path)?,
        expected,
        &path.display().to_string(),
    )
}

fn verify_sha256(actual: &str, expected: &str, subject: &str) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(XtaskError::HashMismatch {
            subject: subject.to_string(),
            expected: expected.to_string(),
            actual: actual.to_string(),
        })
    }
}

fn extract_expected_file(
    archive_bytes: &[u8],
    entry: &FfmpegBinaryEntry,
    destination: &Path,
) -> Result<()> {
    let reader = Cursor::new(archive_bytes);
    let mut archive = zip::ZipArchive::new(reader)?;

    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let matches =
            file.is_file() && archive_entry_name_matches(&file.name()?, entry.expected_names);
        if matches {
            write_archive_file(&mut file, destination, entry.sha256, entry.make_executable)?;
            println!("Placed {}.", entry.destination_name);
            return Ok(());
        }
    }

    Err(XtaskError::ArchiveEntryMissing {
        expected_names: entry.expected_names.join(", "),
    })
}

fn archive_entry_name_matches(name: &str, expected_names: &[&str]) -> bool {
    let file_name = name.rsplit(['/', '\\']).next().unwrap_or(name);
    expected_names.contains(&file_name)
}

fn write_archive_file(
    reader: &mut impl Read,
    destination: &Path,
    expected_sha256: &str,
    make_executable: bool,
) -> Result<()> {
    let Some(file_name) = destination.file_name().and_then(|name| name.to_str()) else {
        return Err(XtaskError::Usage(format!(
            "invalid destination path `{}`",
            destination.display()
        )));
    };

    let temporary_destination = destination.with_file_name(format!(".{file_name}.download"));
    {
        let mut output = fs::File::create(&temporary_destination)?;
        io::copy(reader, &mut output)?;
    }

    if let Err(error) = verify_file_sha256(&temporary_destination, expected_sha256) {
        let _ = fs::remove_file(&temporary_destination);
        return Err(error);
    }

    if make_executable {
        make_file_executable(&temporary_destination)?;
    }

    if destination.exists() {
        fs::remove_file(destination)?;
    }
    fs::rename(temporary_destination, destination)?;
    Ok(())
}

#[cfg(unix)]
fn make_file_executable(path: &Path) -> Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_file_executable(_path: &Path) -> Result<()> {
    Ok(())
}

fn host_platform() -> &'static str {
    match env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    }
}

const fn host_arch() -> &'static str {
    env::consts::ARCH
}

fn ci() -> Result<()> {
    check_theme_color_literals()?;
    run_command("cargo", &["fmt", "--all", "--check"])?;
    run_command("cargo", &["test", "--workspace", "--locked"])?;
    run_command(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--locked",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run_command("git", &["diff", "--check"])?;
    Ok(())
}

const LEGACY_THEME_TOKENS: [&str; 12] = [
    "BACKGROUND",
    "FOREGROUND",
    "TRANSPARENT",
    "SIDEBAR",
    "DROPDOWN",
    "FRAME_GRAY_100",
    "FRAME_GRAY_200",
    "FRAME_GRAY_400",
    "FRAME_GRAY_600",
    "FRAME_BLUE",
    "FRAME_RED",
    "FRAME_AMBER",
];

struct ApprovedUiColorLiteral {
    path: &'static str,
    function: &'static str,
    constructor: &'static str,
    arguments: &'static [f64],
}

const APPROVED_UI_COLOR_LITERALS: [ApprovedUiColorLiteral; 3] = [
    ApprovedUiColorLiteral {
        path: "frame-app/src/app/preview_panel/crop_overlay.rs",
        function: "crop_mask_rect",
        constructor: "hsla",
        arguments: &[0.0, 0.0, 0.0, 0.55],
    },
    ApprovedUiColorLiteral {
        path: "frame-app/src/app/preview_panel/crop_overlay.rs",
        function: "preview_crop_handle",
        constructor: "hsla",
        arguments: &[0.0, 0.0, 0.0, 0.45],
    },
    ApprovedUiColorLiteral {
        path: "frame-app/src/app/preview_panel/overlay.rs",
        function: "preview_overlay_handle",
        constructor: "hsla",
        arguments: &[0.0, 0.0, 0.0, 0.45],
    },
];

#[derive(Debug, PartialEq)]
struct UiColorLiteralCall {
    function: Option<String>,
    constructor: &'static str,
    arguments: Vec<f64>,
}

#[derive(Debug, Default, PartialEq)]
struct ThemeColorAudit {
    legacy_tokens: BTreeSet<String>,
    color_literals: Vec<UiColorLiteralCall>,
}

#[derive(Default)]
struct ThemeColorVisitor {
    current_function: Option<String>,
    audit: ThemeColorAudit,
}

impl<'ast> Visit<'ast> for ThemeColorVisitor {
    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        let identifier = ident.to_string();
        if LEGACY_THEME_TOKENS.contains(&identifier.as_str()) {
            self.audit.legacy_tokens.insert(identifier);
        }
    }

    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        let previous = self
            .current_function
            .replace(function.sig.ident.to_string());
        visit::visit_item_fn(self, function);
        self.current_function = previous;
    }

    fn visit_impl_item_fn(&mut self, function: &'ast ImplItemFn) {
        let previous = self
            .current_function
            .replace(function.sig.ident.to_string());
        visit::visit_impl_item_fn(self, function);
        self.current_function = previous;
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let Some((constructor, arguments)) = numeric_color_literal(call) {
            self.audit.color_literals.push(UiColorLiteralCall {
                function: self.current_function.clone(),
                constructor,
                arguments,
            });
        }
        visit::visit_expr_call(self, call);
    }
}

fn numeric_color_literal(call: &syn::ExprCall) -> Option<(&'static str, Vec<f64>)> {
    let constructor = audited_color_constructor(&call.func)?;
    let arguments = call
        .args
        .iter()
        .map(numeric_literal_value)
        .collect::<Option<Vec<_>>>()?;
    (!arguments.is_empty()).then_some((constructor, arguments))
}

fn audited_color_constructor(callee: &Expr) -> Option<&'static str> {
    let Expr::Path(path) = callee else {
        return None;
    };
    let last = path.path.segments.last()?.ident.to_string();
    match last.as_str() {
        "rgb" => Some("rgb"),
        "rgba" => Some("rgba"),
        "hsla" => Some("hsla"),
        "from_rgb"
            if path
                .path
                .segments
                .iter()
                .rev()
                .nth(1)
                .is_some_and(|segment| segment.ident == "RgbaToken") =>
        {
            Some("RgbaToken::from_rgb")
        }
        "new"
            if path
                .path
                .segments
                .iter()
                .rev()
                .nth(1)
                .is_some_and(|segment| segment.ident == "OklchToken") =>
        {
            Some("OklchToken::new")
        }
        _ => None,
    }
}

fn numeric_literal_value(expression: &Expr) -> Option<f64> {
    match expression {
        Expr::Lit(literal) => match &literal.lit {
            Lit::Int(value) => value.base10_digits().parse().ok(),
            Lit::Float(value) => value.base10_digits().parse().ok(),
            _ => None,
        },
        Expr::Unary(unary) if matches!(&unary.op, UnOp::Neg(_)) => {
            numeric_literal_value(&unary.expr).map(|value| -value)
        }
        Expr::Group(group) => numeric_literal_value(&group.expr),
        Expr::Paren(parenthesized) => numeric_literal_value(&parenthesized.expr),
        _ => None,
    }
}

fn analyze_theme_color_source(source: &str) -> syn::Result<ThemeColorAudit> {
    let file = syn::parse_file(source)?;
    let mut visitor = ThemeColorVisitor::default();
    visitor.visit_file(&file);
    Ok(visitor.audit)
}

fn approved_ui_color_literal_index(path: &str, literal: &UiColorLiteralCall) -> Option<usize> {
    APPROVED_UI_COLOR_LITERALS.iter().position(|approved| {
        approved.path == path
            && literal.function.as_deref() == Some(approved.function)
            && literal.constructor == approved.constructor
            && literal.arguments == approved.arguments
    })
}

fn check_theme_color_literals() -> Result<()> {
    let root = repo_root()?;
    let mut source_paths = Vec::new();
    collect_rust_sources(&root.join("frame-app/src"), &mut source_paths)?;
    source_paths.sort();

    let mut approved_counts = vec![0_usize; APPROVED_UI_COLOR_LITERALS.len()];
    let mut violations = Vec::new();

    for path in source_paths {
        let source = fs::read_to_string(&path)?;
        let relative_path = path
            .strip_prefix(&root)
            .expect("theme audit sources should live inside the repository")
            .to_string_lossy()
            .replace('\\', "/");
        let audit = match analyze_theme_color_source(&source) {
            Ok(audit) => audit,
            Err(error) => {
                violations.push(format!(
                    "{relative_path}: failed to parse Rust source: {error}"
                ));
                continue;
            }
        };

        for token in audit.legacy_tokens {
            violations.push(format!("{relative_path}: legacy theme token `{token}`"));
        }

        if !relative_path.starts_with("frame-app/src/app/") {
            continue;
        }
        for literal in audit.color_literals {
            if let Some(index) = approved_ui_color_literal_index(&relative_path, &literal) {
                approved_counts[index] += 1;
            } else {
                let function = literal.function.as_deref().unwrap_or("<module>");
                let arguments = literal
                    .arguments
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                violations.push(format!(
                    "{relative_path}: unapproved {}({arguments}) in `{function}`",
                    literal.constructor
                ));
            }
        }
    }

    for (approved, actual_count) in APPROVED_UI_COLOR_LITERALS.iter().zip(approved_counts) {
        if actual_count != 1 {
            violations.push(format!(
                "{}: expected one approved {} literal in `{}`, found {actual_count}",
                approved.path, approved.constructor, approved.function
            ));
        }
    }

    if violations.is_empty() {
        println!("Theme color audit passed: no legacy tokens or unapproved UI literals.");
        Ok(())
    } else {
        Err(XtaskError::ThemeColorAudit {
            details: violations.join("\n"),
        })
    }
}

fn collect_rust_sources(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_rust_sources(&path, paths)?;
        } else if path.extension().and_then(OsStr::to_str) == Some("rs") {
            paths.push(path);
        }
    }
    Ok(())
}

fn run_command(program: &str, args: &[&str]) -> Result<()> {
    let root = repo_root()?;
    println!("$ {} {}", program, args.join(" "));
    let status = Command::new(program)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|source| XtaskError::CommandSpawn {
            program: program.to_string(),
            source,
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(XtaskError::CommandFailed {
            program: program.to_string(),
            status,
        })
    }
}

fn run_command_path(program: impl AsRef<OsStr>, args: &[String]) -> Result<()> {
    let program = program.as_ref();
    println!(
        "$ {} {}",
        program.to_string_lossy(),
        args.iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" ")
    );
    let status = Command::new(program)
        .args(args)
        .current_dir(repo_root()?)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|source| XtaskError::CommandSpawn {
            program: program.to_string_lossy().into_owned(),
            source,
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(XtaskError::CommandFailed {
            program: program.to_string_lossy().into_owned(),
            status,
        })
    }
}

fn repo_root() -> Result<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or(XtaskError::RepoRoot)
}

#[derive(Debug)]
enum XtaskError {
    ArchiveEntryMissing {
        expected_names: String,
    },
    CommandFailed {
        program: String,
        status: std::process::ExitStatus,
    },
    CommandSpawn {
        program: String,
        source: io::Error,
    },
    Download {
        url: String,
        source: Box<ureq::Error>,
    },
    HashMismatch {
        subject: String,
        expected: String,
        actual: String,
    },
    Help,
    Io(io::Error),
    RepoRoot,
    ThemeColorAudit {
        details: String,
    },
    Usage(String),
    Zip(zip::result::ZipError),
}

impl fmt::Display for XtaskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArchiveEntryMissing { expected_names } => {
                write!(
                    formatter,
                    "archive did not contain expected file: {expected_names}"
                )
            }
            Self::CommandFailed { program, status } => {
                write!(formatter, "`{program}` failed with status {status}")
            }
            Self::CommandSpawn { program, source } => {
                write!(formatter, "failed to run `{program}`: {source}")
            }
            Self::Download { url, source } => {
                write!(formatter, "failed to download `{url}`: {source}")
            }
            Self::HashMismatch {
                subject,
                expected,
                actual,
            } => write!(
                formatter,
                "SHA-256 mismatch for `{subject}`: expected {expected}, got {actual}"
            ),
            Self::Help => Ok(()),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::RepoRoot => write!(formatter, "failed to resolve repository root"),
            Self::ThemeColorAudit { details } => {
                write!(formatter, "theme color audit failed:\n{details}")
            }
            Self::Usage(message) => write!(formatter, "{message}"),
            Self::Zip(error) => write!(formatter, "failed to read zip archive: {error}"),
        }
    }
}

impl From<io::Error> for XtaskError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<zip::result::ZipError> for XtaskError {
    fn from(error: zip::result::ZipError) -> Self {
        Self::Zip(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_color_audit_accepts_current_source_inventory() {
        check_theme_color_literals().expect("theme color inventory should remain allowlisted");
    }

    #[test]
    fn theme_color_audit_ignores_legacy_names_in_comments_and_strings() {
        let audit = analyze_theme_color_source(
            r#"
            // BACKGROUND is only documentation.
            fn example() {
                let _label = "FRAME_BLUE";
            }
            "#,
        )
        .unwrap();

        assert!(audit.legacy_tokens.is_empty());
    }

    #[test]
    fn theme_color_audit_detects_exact_legacy_identifiers() {
        let audit = analyze_theme_color_source(
            r"
            fn example() {
                let _color = BACKGROUND;
            }
            ",
        )
        .unwrap();

        assert_eq!(
            audit.legacy_tokens,
            BTreeSet::from(["BACKGROUND".to_string()])
        );
    }

    #[test]
    fn theme_color_audit_catches_numeric_token_constructors_but_not_dynamic_colors() {
        let audit = analyze_theme_color_source(
            r"
            fn example(red: u8, green: u8, blue: u8) {
                let _hardcoded = RgbaToken::from_rgb(255, 255, 255);
                let _dynamic = RgbaToken::from_rgb(red, green, blue);
            }
            ",
        )
        .unwrap();

        assert_eq!(
            audit.color_literals,
            vec![UiColorLiteralCall {
                function: Some("example".to_string()),
                constructor: "RgbaToken::from_rgb",
                arguments: vec![255.0, 255.0, 255.0],
            }]
        );
    }

    #[test]
    fn theme_color_allowlist_requires_the_exact_function_context() {
        let audit = analyze_theme_color_source(
            r"
            fn wrong_function() {
                let _overlay = hsla(0.0, 0.0, 0.0, 0.55);
            }
            ",
        )
        .unwrap();
        let literal = audit.color_literals.first().unwrap();

        assert_eq!(
            approved_ui_color_literal_index(
                "frame-app/src/app/preview_panel/crop_overlay.rs",
                literal,
            ),
            None
        );
    }

    #[test]
    fn setup_ffmpeg_options_parse_platform_arch_and_force() {
        let args = vec![
            "--force".to_string(),
            "--platform".to_string(),
            "darwin".to_string(),
            "--arch".to_string(),
            "aarch64".to_string(),
        ];

        let options = SetupFfmpegOptions::parse(&args).unwrap();

        assert_eq!(
            options,
            SetupFfmpegOptions {
                force: true,
                platform: Some("darwin".to_string()),
                arch: Some("aarch64".to_string()),
            }
        );
    }

    #[test]
    fn ffmpeg_target_for_maps_macos_arm64_to_darwin_runtime_names() {
        let target = ffmpeg_target_for("darwin", "arm64").unwrap();

        let FfmpegTarget::Individual { binaries, .. } = target else {
            panic!("expected individual macOS binaries");
        };

        let names = binaries
            .iter()
            .map(|entry| entry.destination_name)
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "ffmpeg-aarch64-apple-darwin",
                "ffprobe-aarch64-apple-darwin"
            ]
        );
    }

    #[test]
    fn ffmpeg_target_for_maps_windows_x64_to_shared_archive() {
        let target = ffmpeg_target_for("win32", "x64").unwrap();

        let FfmpegTarget::SharedArchive { entries, .. } = target else {
            panic!("expected shared Windows archive");
        };

        let names = entries
            .iter()
            .map(|entry| entry.destination_name)
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "ffmpeg-x86_64-pc-windows-msvc.exe",
                "ffprobe-x86_64-pc-windows-msvc.exe"
            ]
        );
    }

    #[test]
    fn archive_entry_name_matches_nested_zip_paths() {
        assert!(archive_entry_name_matches(
            "ffmpeg-n8.1.2-21-gce3c09c101-win64-gpl-8.1/bin/ffprobe.exe",
            &["ffprobe.exe"],
        ));
    }

    #[test]
    fn windows_ffmpeg_archive_uses_retained_monthly_snapshot() {
        assert_eq!(
            WINDOWS_X86_64_ARCHIVE.url,
            "https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-06-30-13-34/ffmpeg-n8.1.2-21-gce3c09c101-win64-gpl-8.1.zip"
        );
    }

    #[test]
    fn verify_bytes_sha256_accepts_matching_digest() {
        let result = verify_bytes_sha256(
            b"abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            "test bytes",
        );

        assert!(result.is_ok(), "unexpected error: {result:?}");
    }

    #[test]
    fn verify_bytes_sha256_rejects_mismatched_digest() {
        let error = verify_bytes_sha256(b"abc", &"0".repeat(64), "test bytes").unwrap_err();

        assert!(matches!(error, XtaskError::HashMismatch { .. }));
    }

    #[test]
    fn ffmpeg_binary_needs_download_accepts_verified_cache() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("ffmpeg");
        fs::write(&destination, b"abc").unwrap();
        let entry =
            test_ffmpeg_entry("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");

        let needs_download = ffmpeg_binary_needs_download(&entry, &destination, false).unwrap();

        assert!(!needs_download);
    }

    #[test]
    fn ffmpeg_binary_needs_download_rejects_corrupt_cache() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("ffmpeg");
        fs::write(&destination, b"corrupt").unwrap();
        let entry =
            test_ffmpeg_entry("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");

        let needs_download = ffmpeg_binary_needs_download(&entry, &destination, false).unwrap();

        assert!(needs_download);
    }

    #[test]
    fn write_archive_file_preserves_destination_on_hash_mismatch() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("ffmpeg");
        fs::write(&destination, b"verified binary").unwrap();
        let mut downloaded = Cursor::new(b"corrupt binary");

        let result = write_archive_file(&mut downloaded, &destination, &"0".repeat(64), false);

        assert!(
            result.is_err() && fs::read(destination).unwrap() == b"verified binary",
            "a rejected download must not replace the existing binary"
        );
    }

    #[test]
    fn all_ffmpeg_sources_are_pinned_to_version_8_1_2() {
        let individual_archives = [
            MACOS_X86_64_BINARIES,
            MACOS_AARCH64_BINARIES,
            LINUX_X86_64_BINARIES,
            LINUX_AARCH64_BINARIES,
        ]
        .into_iter()
        .flat_map(|entries| entries.iter().filter_map(|entry| entry.archive));
        let archives = individual_archives.chain([WINDOWS_X86_64_ARCHIVE]);

        assert!(archives.into_iter().all(|archive| {
            archive.url.contains(FFMPEG_VERSION)
                && !archive.url.contains("latest")
                && is_sha256(archive.sha256)
        }));
    }

    #[test]
    fn all_ffmpeg_binaries_have_pinned_sha256() {
        let binaries = [
            MACOS_X86_64_BINARIES,
            MACOS_AARCH64_BINARIES,
            LINUX_X86_64_BINARIES,
            LINUX_AARCH64_BINARIES,
            WINDOWS_X86_64_BINARIES,
        ]
        .into_iter()
        .flatten();

        assert!(binaries.into_iter().all(|entry| is_sha256(entry.sha256)));
    }

    fn is_sha256(value: &str) -> bool {
        value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
    }

    fn test_ffmpeg_entry(sha256: &'static str) -> FfmpegBinaryEntry {
        FfmpegBinaryEntry {
            id: "ffmpeg",
            archive: None,
            expected_names: &["ffmpeg"],
            destination_name: "ffmpeg",
            sha256,
            make_executable: false,
        }
    }
}
