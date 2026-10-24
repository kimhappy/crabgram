//! Assemble, check and deploy the LV2 bundle of a crabgram plugin, or build its standalone app,
//! from an xtask.
//!
//! Describe the plugin once as a [`Project`]. [`run`] gives the xtask these commands, and a
//! [`Prepare`] step of its [`Assets`] one more:
//!
//! | Command | Does |
//! | - | - |
//! | `bundle` | device: LV2 bundle; native: standalone app |
//! | `check` | checks the styling, images and generated files; device also runs `sord_validate`, `lv2lint`, the ABI check |
//! | `deploy` | device only: runs `check`, installs the bundle over SSH and restarts the plugin services |
//! | `help` | lists the commands |
//!
//! ```no_run
//! use crabgram_build::Project;
//! # mod crabgram_gain {
//! #     use crabgram::{Context, Controls, Info, Plugin};
//! #     #[derive(Controls)] pub struct Params { gain: f32 }
//! #     pub struct Gain;
//! #     impl Plugin for Gain {
//! #         type Controls = Params;
//! #         const INFO: Info = crabgram::info! {
//! #             uri: c"urn:example:gain",
//! #             name: "Example Gain",
//! #             description: "Gain.",
//! #             abbreviation: "GN",
//! #             category: Utility,
//! #             channels: 1,
//! #         };
//! #         fn new() -> Self { Self }
//! #         fn run(&mut self, _: &Params, _: &mut Context<'_, Self>) {}
//! #     }
//! # }
//!
//! struct Xtask;
//!
//! impl Project for Xtask {
//!     type Plugin = crabgram_gain::Gain;
//! }
//!
//! fn main() -> std::process::ExitCode {
//!     crabgram_build::run::<Xtask>()
//! }
//! ```
//!
//! # Workflow
//!
//! ```sh
//! nix develop --command cargo <alias> bundle            # native: standalone app in build/<package>/
//! nix develop .#anagram --command cargo <alias> bundle  # device: LV2 bundle
//! nix develop .#anagram --command cargo <alias> deploy  # `--host <ssh destination>` for another host
//! ```
//!
//! - Nix reads the project through git, so `flake.nix` and `flake.lock` must be tracked: run
//!   `git add -N flake.nix` before the first `nix develop` and `git add flake.lock` after it.
//! - To extend both shells, call `crabgram.lib.devShells (pkgs: { packages = [ pkgs.python3 ]; })`.
//! - For a local crabgram checkout, name it in the flake by its path or `git+file:`, not `path:`,
//!   which copies `target/`, and patch the crates in `.cargo/config.toml`, not with `--config`.
//! - The native `bundle` builds the `crabgram-standalone` of the same crabgram checkout.
//!
//! # Images
//!
//! The bundle ships the images of [`Project::ASSETS`] in [`ASSET_DIRECTORY`]: the styling path
//! `asset/knob.png` names `knob.png` there. A [`Prepare`] step that draws them writes each with
//! [`write_asset`]; [`problems`] checks them.
//!
//! # Environment
//!
//! The Nix shells of the crabgram flake set these, but for `CARGO`, which cargo sets.
//!
//! | Variable | Used by | Holds |
//! | - | - | - |
//! | `BUNDLE_DIR` | `bundle`, `check`, `deploy` | the bundle directory, absolute or relative to the workspace root |
//! | `ANAGRAM_TARGET` | `bundle`, `check`, `deploy` | the device target triple, built with `cargo zigbuild`; unset for native |
//! | `PATCHELF_TOOL` | `bundle` (device) | `patchelf` |
//! | `LV2_BUNDLES` | `check` (device) | a directory of the ontologies the descriptions use |
//! | `SORD_VALIDATE_TOOL` | `check` (device) | `sord_validate` |
//! | `LV2LINT_TOOL` | `check` (device) | `lv2lint` |
//! | `LV2_EMULATOR` | `check` (device) | runs a device binary on the build host |
//! | `CARGO` | every command | the cargo that runs the xtask |
//! | `READELF_TOOL` | `check`, `deploy` (device), for the ABI check | `readelf` |
//! | `DEVICE_GLIBC_VERSION` | `bundle`, `check`, `deploy` (device) | the glibc version to link against |
//! | `NICKEL_LIB_DIR` | the build script of `crabgram` (device, licensed) | the directory of `libnickel.a` |
//! | `NICKEL_INCLUDE_DIR` | the build script of `crabgram` (device, licensed) | the directory of `libnickel.h` |
//! | `LV2_INCLUDE_DIR` | the build script of `crabgram` (device, licensed) | the directory of the LV2 headers |
//! | `LIBCLANG_PATH` | the build script of `crabgram` (device, licensed) | the directory of `libclang`, which binds the header |
//! | `HEADING_FONT`, `MONO_FONT`, `MONO_MEDIUM_FONT` | the compilation of `crabgram-standalone` (native `bundle`) | the TTF fonts |
//! | `ALSA_PLUGIN_DIR` | the standalone app at run time (Linux); set it where the app runs outside the shell | the ALSA plugin directory |

#![feature(error_reporter)]

mod abi;
mod assets;
mod check;
mod deploy;
mod images;
mod problem;
mod standalone;
mod tools;
mod turtle;

#[cfg(test)]
mod fixture;

use std::{
    env,
    error::Report,
    ffi::OsString,
    fs, io, iter,
    num::NonZeroU32,
    path::{Path, PathBuf},
    process::{self, ExitCode},
};

use crabgram::{Control, Controls, Info, Plugin};
use crabgram_styling::{
    Styling,
    ui::{self, Model},
};

use crate::{
    abi::AbiError,
    assets::asset_file,
    problem::ensure_sound,
    tools::{ToolError, Workspace, tool},
};
pub use crate::{
    assets::write_asset,
    problem::{BundleProblem, Problems, problems},
};

/// The bundle directory of the images.
pub const ASSET_DIRECTORY: &str = "asset";

/// A command [`run`] gives every xtask.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Builtin {
    /// `bundle`.
    Bundle,
    /// `check`.
    Check,
    /// `deploy`.
    Deploy,
}

impl Builtin {
    const ALL: [Self; 3] = [Self::Bundle, Self::Check, Self::Deploy];

    /// The name that runs it.
    const fn name(self) -> &'static str {
        match self {
            Self::Bundle => "bundle",
            Self::Check => "check",
            Self::Deploy => "deploy",
        }
    }

    const fn synopsis(self) -> &'static str {
        match self {
            Self::Bundle | Self::Check => self.name(),
            Self::Deploy => "deploy [--host <ssh destination>]",
        }
    }

    fn summary(self) -> String {
        match self {
            Self::Bundle => {
                "builds the plugin and assembles the LV2 bundle or the standalone app".to_owned()
            }
            Self::Check => {
                "checks the bundle against the plugin, and a device one against the LV2 tools"
                    .to_owned()
            }
            Self::Deploy => format!(
                "checks the device bundle, installs it on {} or the --host destination and \
                 restarts the plugin services",
                deploy::DEVICE_HOST
            ),
        }
    }
}

type PrepareFn = fn(&Path) -> io::Result<()>;

/// A command of the xtask's own, such as drawing the images, given the asset directory: [`run`]
/// runs it alone by its name, and before `bundle` or the commands of
/// [`with_before`](Self::with_before).
///
/// ```
/// use crabgram_build::{Builtin, Prepare};
///
/// const ART: Prepare = Prepare::new("art", |assets| std::fs::create_dir_all(assets))
///     .with_before(&[Builtin::Bundle, Builtin::Deploy]);
/// ```
///
/// A command named after a builtin or `help`, or starting with `-`, fails to compile:
///
/// ```compile_fail,E0080
/// const BUNDLE: crabgram_build::Prepare = crabgram_build::Prepare::new("bundle", |_| Ok(()));
///
/// let _ = BUNDLE;
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Prepare {
    command: &'static str,
    before: &'static [Builtin],
    run: PrepareFn,
}

impl Prepare {
    /// `run` as the command `command`, and before `bundle`.
    ///
    /// # Panics
    ///
    /// When `command` is `bundle`, `check`, `deploy`, `help` or starts with `-`; in a `const`,
    /// that fails to compile.
    #[must_use]
    pub const fn new(command: &'static str, run: fn(&Path) -> io::Result<()>) -> Self {
        assert!(
            !matches!(
                command.as_bytes(),
                b"bundle" | b"check" | b"deploy" | b"help" | [b'-', ..]
            ),
            "a prepare command cannot take the name of a builtin command or start with `-`"
        );

        Self {
            command,
            before: &[Builtin::Bundle],
            run,
        }
    }

    /// Runs it before `commands` instead.
    ///
    /// # Panics
    ///
    /// When `commands` lists `check` or `deploy` but not `bundle`, which would find the bundle
    /// stale; in a `const`, that fails to compile:
    ///
    /// ```compile_fail,E0080
    /// use crabgram_build::{Builtin, Prepare};
    ///
    /// const ART: Prepare = Prepare::new("art", |_| Ok(())).with_before(&[Builtin::Check]);
    ///
    /// let _ = ART;
    /// ```
    #[must_use]
    pub const fn with_before(self, commands: &'static [Builtin]) -> Self {
        assert!(
            lists(commands, Builtin::Bundle)
                || !(lists(commands, Builtin::Check) || lists(commands, Builtin::Deploy)),
            "a prepare step that runs before check or deploy must run before bundle too, or they \
             find the bundle stale"
        );

        Self {
            before: commands,
            ..self
        }
    }
}

const fn lists(commands: &[Builtin], builtin: Builtin) -> bool {
    match commands {
        [] => false,
        [first, rest @ ..] => *first as u8 == builtin as u8 || lists(rest, builtin),
    }
}

/// The directory of the images the styling names, and the step that draws them there, if any.
///
/// ```
/// use crabgram_build::{Assets, Prepare};
///
/// const ASSETS: Assets = crabgram_build::assets!("../asset")
///     .with_prepare(Prepare::new("art", |assets| {
///         std::fs::create_dir_all(assets)
///     }));
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Assets {
    directory: &'static str,
    prepare: Option<Prepare>,
}

impl Assets {
    /// The images in the absolute `directory`, with no step that draws them: [`assets!`] calls it.
    #[doc(hidden)]
    #[must_use]
    pub const fn at_absolute(directory: &'static str) -> Self {
        Self {
            directory,
            prepare: None,
        }
    }

    /// Draws the images with `prepare` first.
    #[must_use]
    pub const fn with_prepare(self, prepare: Prepare) -> Self {
        Self {
            prepare: Some(prepare),
            ..self
        }
    }
}

/// A plugin as the xtask sees it.
pub trait Project {
    /// The plugin type the `cdylib` exports.
    type Plugin: Plugin;

    /// Where the images the styling names live. Default: none.
    const ASSETS: Option<Assets> = None;

    /// Default: none, the stock look.
    #[must_use]
    fn styling() -> Styling {
        Styling::default()
    }
}

/// The [`Assets`] in `relative`, a directory of the package that calls it.
///
/// ```
/// const ASSETS: crabgram_build::Assets = crabgram_build::assets!("../asset");
/// ```
///
/// An absolute path fails to compile:
///
/// ```compile_fail
/// const ASSETS: crabgram_build::Assets = crabgram_build::assets!("/asset");
/// ```
#[macro_export]
macro_rules! assets {
    ($relative:literal) => {{
        const {
            ::core::assert!(
                !::core::matches!($relative.as_bytes(), [b'/', ..]),
                "`assets!` takes a path relative to the package, not an absolute one"
            );
        }

        $crate::Assets::at_absolute(::core::concat!(
            ::core::env!("CARGO_MANIFEST_DIR"),
            "/",
            $relative
        ))
    }};
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("usage: <{}bundle|check|deploy [--host <ssh destination>]|help>", prepare.map_or_else(String::new, |it| format!("{it}|")))]
    Usage { prepare: Option<&'static str> },
    #[error("unknown command `{command}`; run `help`")]
    UnknownCommand { command: String },
    #[error("{command} failed")]
    Prepare {
        command: &'static str,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Tool(#[from] ToolError),
    #[error(transparent)]
    Abi(#[from] AbiError),
    #[error("the styling or its images are unsound:\n  {}", problems.to_string().replace('\n', "\n  "))]
    Unsound { problems: Problems },
    #[error("{action} {}", path.display())]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("serializing the UI model")]
    Serialize(#[source] serde_json::Error),
    #[error("no bundle at {}, run the bundle command first", directory.display())]
    NotBundled { directory: PathBuf },
    #[error(
        "only a device build can be deployed; run it in the device shell, `nix develop .#anagram`"
    )]
    NativeDeploy,
    #[error(
        "--host takes an ssh destination that is not empty and does not start with `-`, not {host:?}"
    )]
    Host { host: String },
    #[error(
        "the bundle at {} no longer matches the plugin ({} missing or changed): bundle it again",
        directory.display(),
        files.join(", ")
    )]
    Stale {
        directory: PathBuf,
        files: Vec<String>,
    },
    #[error("the binary at {} is older than its code: bundle it again", binary.display())]
    StaleBinary { binary: PathBuf },
    #[error("the binary at {} is missing: bundle it again", binary.display())]
    MissingBinary { binary: PathBuf },
}

fn io_error(action: &'static str, path: &Path) -> impl FnOnce(io::Error) -> Error {
    move |source| Error::Io {
        action,
        path: path.to_owned(),
        source,
    }
}

/// Runs the command named by the arguments, after the [`Prepare`] step it needs.
///
/// Prints any error and returns failure; `help` lists the commands.
///
/// ```no_run
/// # struct Xtask;
/// # impl crabgram_build::Project for Xtask { type Plugin = Gain; }
/// # use crabgram::{Context, Controls, Info, Plugin};
/// # #[derive(Controls)] pub struct Params { gain: f32 }
/// # pub struct Gain;
/// # impl Plugin for Gain {
/// #     type Controls = Params;
/// #     const INFO: Info = crabgram::info! {
/// #         uri: c"urn:example:gain",
/// #         name: "Example Gain",
/// #         description: "Gain.",
/// #         abbreviation: "GN",
/// #         category: Utility,
/// #         channels: 1,
/// #     };
/// #     fn new() -> Self { Self }
/// #     fn run(&mut self, _: &Params, _: &mut Context<'_, Self>) {}
/// # }
/// fn main() -> std::process::ExitCode {
///     crabgram_build::run::<Xtask>()
/// }
/// ```
pub fn run<P: Project>() -> ExitCode {
    let arguments = env::args_os()
        .skip(1)
        .map(OsString::into_string)
        .collect::<Result<Vec<_>, _>>();

    match arguments
        .map_err(|_| usage::<P>())
        .and_then(|it| command::<P>(&it))
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xtask: {}", Report::new(error));
            ExitCode::FAILURE
        }
    }
}

fn usage<P: Project>() -> Error {
    Error::Usage {
        prepare: prepare_of::<P>().map(|it| it.command),
    }
}

fn asset_directory<P: Project>() -> Option<&'static Path> {
    P::ASSETS.map(|it| Path::new(it.directory))
}

fn prepare_of<P: Project>() -> Option<Prepare> {
    P::ASSETS.and_then(|it| it.prepare)
}

/// Runs the [`Prepare`] step of `P` in its asset directory, if it has one.
fn prepare<P: Project>() -> Result<(), Error> {
    P::ASSETS
        .and_then(|assets| assets.prepare.zip(Some(assets.directory)))
        .map_or(Ok(()), |(prepare, directory)| {
            (prepare.run)(Path::new(directory)).map_err(|source| Error::Prepare {
                command: prepare.command,
                source,
            })
        })
}

/// A command of the xtask.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Command {
    Help,
    Prepare,
    Bundle,
    Check,
    Deploy { host: String },
}

impl Command {
    /// Whether `prepare` runs first: for its own command and those it runs before.
    fn prepared_by(&self, prepare: &Prepare) -> bool {
        let before = |builtin| prepare.before.contains(&builtin);

        match self {
            Self::Help => false,
            Self::Prepare => true,
            Self::Bundle => before(Builtin::Bundle),
            Self::Check => before(Builtin::Check),
            Self::Deploy { .. } => before(Builtin::Deploy),
        }
    }

    /// The command of a command line, parsed before anything runs.
    fn parse<P: Project>(arguments: &[String]) -> Result<Self, Error> {
        let prepare = prepare_of::<P>().map(|it| it.command);
        let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();

        match arguments[..] {
            ["help"] => Ok(Self::Help),
            ["bundle"] => Ok(Self::Bundle),
            ["check"] => Ok(Self::Check),
            ["deploy"] => Ok(Self::Deploy {
                host: deploy::DEVICE_HOST.to_owned(),
            }),
            ["deploy", "--host", host] => Ok(Self::Deploy {
                host: deploy::destination(host)?,
            }),
            [name] if Some(name) == prepare => Ok(Self::Prepare),
            [name] if !Self::known(name, prepare) => Err(Error::UnknownCommand {
                command: name.to_owned(),
            }),
            _ => Err(usage::<P>()),
        }
    }

    fn known(name: &str, prepare: Option<&str>) -> bool {
        Builtin::ALL.iter().any(|it| it.name() == name) || name == "help" || Some(name) == prepare
    }
}

fn command<P: Project>(arguments: &[String]) -> Result<(), Error> {
    let command = Command::parse::<P>(arguments)?;

    if prepare_of::<P>().is_some_and(|it| command.prepared_by(&it)) {
        prepare::<P>()?;
    }

    match command {
        Command::Help => {
            println!("{}", help::<P>());
            Ok(())
        }
        Command::Prepare => Ok(()),
        Command::Bundle => bundle::<P>(),
        Command::Check => check::check::<P>(),
        Command::Deploy { host } => deploy::deploy::<P>(&host),
    }
}

/// One line per command: its synopsis, then what it does.
fn help<P: Project>() -> String {
    let prepare = prepare_of::<P>().map(|it| {
        let before = it
            .before
            .iter()
            .map(|builtin| builtin.name())
            .collect::<Vec<_>>()
            .join(", ");

        (
            it.command,
            if before.is_empty() {
                "runs the xtask's own step".to_owned()
            } else {
                format!("runs the xtask's own step, also first for {before}")
            },
        )
    });
    let commands = prepare
        .into_iter()
        .chain(Builtin::ALL.map(|it| (it.synopsis(), it.summary())))
        .chain([("help", "lists the commands".to_owned())])
        .collect::<Vec<_>>();
    let width = commands
        .iter()
        .map(|(synopsis, _)| synopsis.len())
        .max()
        .unwrap_or_default();

    iter::once("usage: <command>".to_owned())
        .chain(
            commands
                .iter()
                .map(|(synopsis, summary)| format!("  {synopsis:<width$}  {summary}")),
        )
        .collect::<Vec<_>>()
        .join("\n")
}

const fn info<P: Project>() -> &'static Info {
    const { &<P::Plugin as Plugin>::INFO }
}

const fn package<P: Project>() -> &'static str {
    info::<P>().package()
}

#[derive(Debug)]
enum Target {
    Native,
    Device { triple: String },
}

impl Target {
    fn current() -> Self {
        tools::device_target().map_or(Self::Native, |triple| Self::Device { triple })
    }

    /// The file name of the plugin binary in the output: for the device, the one the description
    /// names.
    fn binary<P: Project>(&self) -> String {
        match self {
            Self::Native => format!("{}.{}", package::<P>(), env::consts::DLL_EXTENSION),
            Self::Device { .. } => info::<P>().binary(),
        }
    }

    /// The extension of the cdylib cargo builds, which the binary keeps.
    fn extension<P: Project>(&self) -> String {
        Path::new(&self.binary::<P>())
            .extension()
            .map_or_else(String::new, |it| it.to_string_lossy().into_owned())
    }

    /// The LV2 bundle for the device, the standalone app directory `<package>` for native.
    fn output_directory<P: Project>(&self, workspace: &Workspace) -> Result<PathBuf, Error> {
        let info = info::<P>();

        Ok(workspace.bundle(&match self {
            Self::Native => info.package().to_owned(),
            Self::Device { .. } => info.bundle(),
        })?)
    }
}

const fn controls<P: Project>() -> &'static [Control] {
    <P::Plugin as Plugin>::Controls::CONTROLS
}

fn create_parent(path: &Path) -> Result<(), Error> {
    path.parent().map_or(Ok(()), |parent| {
        fs::create_dir_all(parent).map_err(io_error("creating", parent))
    })
}

fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), Error> {
    fs::write(path, contents).map_err(io_error("writing", path))
}

fn copy(source: &Path, target: &Path) -> Result<(), Error> {
    fs::copy(source, target).map_err(io_error("copying to", target))?;
    Ok(())
}

fn model<P: Project>(styling: &Styling) -> Model {
    let info = info::<P>();

    Model::new(
        info.uri(),
        info.name(),
        NonZeroU32::new(info.channels()).expect("an info has at least one channel"),
        controls::<P>().to_vec(),
        styling.block_settings().cloned().unwrap_or_default(),
    )
}

fn assembled<P: Project>(workspace: &Workspace, target: &Target) -> Result<PathBuf, Error> {
    let directory = target.output_directory::<P>(workspace)?;

    if directory.is_dir() {
        Ok(directory)
    } else {
        Err(Error::NotBundled { directory })
    }
}

/// Flags of every device build.
const DEVICE_RUSTFLAGS: [&str; 5] = [
    "-Ctarget-cpu=cortex-a76",
    "-Znext-solver=coherence",
    "-Zunstable-options",
    "-Cpanic=immediate-abort",
    "-Alinker-messages",
];

/// `cargo build` natively; for the device, `cargo zigbuild` against the device glibc with
/// [`DEVICE_RUSTFLAGS`] replacing any flags from the environment.
fn build<P: Project>(workspace: &Workspace, target: &Target) -> Result<PathBuf, ToolError> {
    let package_id = workspace.package_id(package::<P>())?;
    let mut command = process::Command::new(tools::cargo()?);

    if let Target::Device { triple } = target {
        command
            .env_remove("RUSTFLAGS")
            .env("CARGO_ENCODED_RUSTFLAGS", DEVICE_RUSTFLAGS.join("\x1f"))
            .args(["zigbuild", "--target"])
            .arg(format!("{triple}.{}", abi::device_glibc()?))
            .args(["-Z", "build-std=std,panic_abort"]);

        if info::<P>().licensing().checks() {
            command.args(["--features", "crabgram/licensing"]);
        }
    } else {
        command.arg("build");
    }

    tools::build_cdylib(
        command
            .current_dir(&workspace.root)
            .args(["--release", "--package", &package_id.repr]),
        package_id,
        &target.extension::<P>(),
    )
}

fn recreate(directory: PathBuf) -> Result<PathBuf, Error> {
    if directory.exists() {
        fs::remove_dir_all(&directory).map_err(io_error("removing", &directory))?;
    }

    fs::create_dir_all(&directory).map_err(io_error("creating", &directory))?;
    Ok(directory)
}

/// For the device, also drops `libgcc_s`; the check command checks the ABI.
fn install_binary<P: Project>(
    artifact: &Path,
    directory: &Path,
    target: &Target,
) -> Result<(), Error> {
    let binary = directory.join(target.binary::<P>());

    copy(artifact, &binary)?;

    if let Target::Device { .. } = target {
        tools::run(
            process::Command::new(tool("PATCHELF_TOOL")?)
                .arg("--remove-rpath")
                .args(["--remove-needed", "libgcc_s.so.1"])
                .arg(&binary),
        )?;
    }

    Ok(())
}

/// The images the styling names, by bundle path.
fn named_images<P: Project>(styling: &Styling) -> Result<Vec<(String, Vec<u8>)>, Error> {
    styling
        .images(controls::<P>())
        .into_keys()
        .map(|path| {
            let source =
                asset_file(asset_directory::<P>(), path).map_err(|problem| Error::Unsound {
                    problems: Problems::from_iter([problem]),
                })?;
            let contents = fs::read(&source).map_err(io_error("reading", &source))?;

            Ok((path.to_owned(), contents))
        })
        .collect()
}

/// The Turtle files of a device bundle, or `ui.json` for native.
fn descriptions<P: Project>(
    styling: &Styling,
    target: &Target,
) -> Result<Vec<(String, Vec<u8>)>, Error> {
    match target {
        Target::Native => {
            let mut json = Vec::new();

            model::<P>(styling)
                .write(&mut json)
                .map_err(Error::Serialize)?;
            Ok(vec![(ui::FILE.to_owned(), json)])
        }
        Target::Device { .. } => Ok(turtle::files::<P::Plugin>(styling)
            .into_iter()
            .map(|(name, text)| (name, text.into_bytes()))
            .collect()),
    }
}

/// Every file of the output besides the binary and the standalone app, by path in it: the
/// descriptions and the images.
fn files<P: Project>(styling: &Styling, target: &Target) -> Result<Vec<(String, Vec<u8>)>, Error> {
    Ok([
        descriptions::<P>(styling, target)?,
        named_images::<P>(styling)?,
    ]
    .concat())
}

fn write_files<P: Project>(
    styling: &Styling,
    directory: &Path,
    target: &Target,
) -> Result<(), Error> {
    files::<P>(styling, target)?
        .iter()
        .try_for_each(|(name, contents)| {
            let path = directory.join(name);

            create_parent(&path)?;
            write(&path, contents)
        })
}

fn assemble<P: Project>(
    workspace: &Workspace,
    styling: &Styling,
    artifact: &Path,
    directory: &Path,
    target: &Target,
) -> Result<(), Error> {
    install_binary::<P>(artifact, directory, target)?;
    write_files::<P>(styling, directory, target)?;

    match target {
        Target::Native => standalone::install::<P>(workspace, directory),
        Target::Device { .. } => Ok(()),
    }
}

/// Builds the plugin and assembles the output. A failed assembly leaves none behind.
fn bundle<P: Project>() -> Result<(), Error> {
    let styling = P::styling();

    ensure_sound::<P>(&styling)?;

    let workspace = Workspace::load()?;
    let target = Target::current();
    let directory = target.output_directory::<P>(&workspace)?;
    let artifact = build::<P>(&workspace, &target)?;
    let directory = recreate(directory)?;

    assemble::<P>(&workspace, &styling, &artifact, &directory, &target).inspect_err(|_| {
        if let Err(cleanup) = fs::remove_dir_all(&directory) {
            eprintln!(
                "xtask: removing the failed bundle {} failed too: {cleanup}",
                directory.display()
            );
        }
    })?;
    println!("bundled {}", directory.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, path::Path};

    use super::{Assets, Builtin, Command, Error, Prepare, Project, Target, command, info};
    use crate::fixture::Gain;

    thread_local! {
        static PREPARED: Cell<u32> = const { Cell::new(0) };
    }

    struct Drawn;

    impl Drawn {
        const ART: Prepare = Prepare::new("art", |assets| {
            assert_eq!(assets, Path::new("/assets"));
            PREPARED.set(PREPARED.get() + 1);
            Ok(())
        });
    }

    impl Project for Drawn {
        type Plugin = Gain;

        const ASSETS: Option<Assets> = Some(Assets::at_absolute("/assets").with_prepare(Self::ART));
    }

    fn owned(arguments: &[&str]) -> Vec<String> {
        arguments.iter().map(|it| (*it).to_owned()).collect()
    }

    fn invoke(arguments: &[&str]) -> Result<(), Error> {
        command::<Drawn>(&owned(arguments))
    }

    #[test]
    fn a_command_line_parses_into_its_command() {
        let parse = |arguments: &[&str]| Command::parse::<Drawn>(&owned(arguments)).ok();
        let deploy = |host: &str| {
            Some(Command::Deploy {
                host: host.to_owned(),
            })
        };

        assert_eq!(parse(&["art"]), Some(Command::Prepare));
        assert_eq!(parse(&["help"]), Some(Command::Help));
        assert_eq!(parse(&["bundle"]), Some(Command::Bundle));
        assert_eq!(parse(&["check"]), Some(Command::Check));
        assert_eq!(parse(&["deploy"]), deploy("root@192.168.51.1"));
        assert_eq!(parse(&["deploy", "--host", "anagram"]), deploy("anagram"));
        assert_eq!(parse(&["deploy", "--host"]), None);
        assert_eq!(parse(&["deploy", "--host=anagram"]), None);
        assert_eq!(parse(&["check", "--host", "anagram"]), None);
        assert_eq!(parse(&["--help"]), None);
    }

    #[test]
    fn a_deploy_host_that_ssh_would_take_for_no_destination_or_an_option_is_refused() {
        let parse = |host: &str| Command::parse::<Drawn>(&owned(&["deploy", "--host", host]));

        assert!(
            ["", "-oProxyCommand=x"]
                .into_iter()
                .all(|host| matches!(parse(host), Err(Error::Host { .. })))
        );
    }

    #[test]
    fn an_unknown_command_is_named() {
        assert_eq!(
            invoke(&["bundel"]).unwrap_err().to_string(),
            "unknown command `bundel`; run `help`"
        );
    }

    #[test]
    fn a_prepare_step_runs_for_its_own_command_and_those_it_runs_before() {
        let commands = [
            Command::Prepare,
            Command::Bundle,
            Command::Check,
            Command::Deploy {
                host: "anagram".to_owned(),
            },
        ];
        let prepared = |prepare: Prepare| commands.each_ref().map(|it| it.prepared_by(&prepare));

        assert_eq!(prepared(Drawn::ART), [true, true, false, false]);
        assert_eq!(
            prepared(Drawn::ART.with_before(&[Builtin::Bundle, Builtin::Check])),
            [true, true, true, false]
        );
    }

    #[test]
    fn the_prepare_command_runs_alone_and_nothing_else_takes_extra_arguments() {
        PREPARED.set(0);

        assert!(invoke(&["art"]).is_ok());
        assert_eq!(PREPARED.get(), 1);
        assert_eq!(
            [
                &["art", "now"][..],
                &["check", "now"],
                &["bundle", "-v"],
                &["deploy", "anagram"],
                &[]
            ]
            .map(|arguments| invoke(arguments).unwrap_err().to_string()),
            ["usage: <art|bundle|check|deploy [--host <ssh destination>]|help>"; 5]
        );
        assert_eq!(PREPARED.get(), 1, "a usage error prepares nothing");
    }

    #[test]
    #[should_panic(expected = "must run before bundle too")]
    fn a_prepare_step_before_check_but_not_bundle_is_refused() {
        let _ = Drawn::ART.with_before(&[Builtin::Check]);
    }

    #[test]
    fn the_device_binary_is_the_one_the_description_names_with_the_extension_cargo_builds() {
        let device = Target::Device {
            triple: "aarch64-unknown-linux-gnu".to_owned(),
        };

        assert_eq!(device.binary::<Drawn>(), info::<Drawn>().binary());
        assert_eq!(device.extension::<Drawn>(), "so");
        assert_eq!(
            Target::Native.extension::<Drawn>(),
            std::env::consts::DLL_EXTENSION
        );
    }

    #[test]
    fn help_lists_every_command_and_prepares_nothing() {
        PREPARED.set(0);

        assert!(invoke(&["help"]).is_ok());
        assert_eq!(PREPARED.get(), 0);
        assert_eq!(
            super::help::<Drawn>(),
            "usage: <command>
  art                                runs the xtask's own step, also first for bundle
  bundle                             builds the plugin and assembles the LV2 bundle or the standalone app
  check                              checks the bundle against the plugin, and a device one against the LV2 tools
  deploy [--host <ssh destination>]  checks the device bundle, installs it on root@192.168.51.1 or the --host destination and restarts the plugin services
  help                               lists the commands"
        );
    }

    #[test]
    fn the_problems_of_a_project_come_after_its_prepare_step() {
        PREPARED.set(0);

        let _ = crate::problems::<Drawn>();

        assert_eq!(PREPARED.get(), 1);
    }
}
