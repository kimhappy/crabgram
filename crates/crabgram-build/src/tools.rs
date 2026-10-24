//! The environment of the Nix shells, and running the tools it names.

use std::{
    env,
    ffi::{OsStr, OsString},
    io, iter,
    path::{Component, Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
};

use cargo_metadata::{Artifact, Message, MetadataCommand, Package, PackageId, camino::Utf8Path};

/// Why cargo or a tool of the environment failed.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("cargo could not describe the workspace")]
    Metadata(#[source] cargo_metadata::Error),
    #[error("the workspace has no package {name}")]
    NoPackage { name: String },
    #[error(
        "{name} must name {what}; the nix dev shell of the project sets it: run inside \
         `nix develop`, or `nix develop .#anagram` for the device"
    )]
    Unset {
        name: &'static str,
        what: &'static str,
    },
    #[error("could not start {line}")]
    Start {
        line: String,
        #[source]
        source: io::Error,
    },
    #[error("{line} exited with {status}")]
    Failed { line: String, status: ExitStatus },
    #[error("reading the messages of cargo")]
    Messages(#[source] io::Error),
    #[error("cargo built no executable of the package")]
    NoExecutable,
    #[error("cargo built no cdylib of the plugin package")]
    NoCdylib,
    #[error(
        "{} holds the sources of the workspace, which assembling a bundle there would delete: \
         point BUNDLE_DIR elsewhere",
        path.display()
    )]
    BundleOverSources { path: PathBuf },
}

impl ToolError {
    pub fn start(line: impl Into<String>) -> impl FnOnce(io::Error) -> Self {
        let line = line.into();

        |source| Self::Start { line, source }
    }
}

fn exited(line: String, status: ExitStatus) -> Result<(), ToolError> {
    if status.success() {
        Ok(())
    } else {
        Err(ToolError::Failed { line, status })
    }
}

pub struct Workspace {
    pub root: PathBuf,
    pub target: PathBuf,
    packages: Vec<Package>,
}

impl Workspace {
    /// The workspace of the package `cargo run` runs, or else of the current directory.
    pub fn load() -> Result<Self, ToolError> {
        let mut command = MetadataCommand::new();

        if let Some(package) = env::var_os("CARGO_MANIFEST_DIR") {
            command.manifest_path(Path::new(&package).join("Cargo.toml"));
        }

        Self::describe(&mut command)
    }

    pub fn of(manifest: &Path) -> Result<Self, ToolError> {
        Self::describe(MetadataCommand::new().manifest_path(manifest))
    }

    fn describe(command: &mut MetadataCommand) -> Result<Self, ToolError> {
        let metadata = command
            .no_deps()
            .verbose(true)
            .exec()
            .map_err(ToolError::Metadata)?;

        Ok(Self {
            root: metadata.workspace_root.into(),
            target: metadata.target_directory.into(),
            packages: metadata.packages,
        })
    }

    pub fn package_id(&self, name: &str) -> Result<&PackageId, ToolError> {
        self.packages
            .iter()
            .find(|it| it.name == name)
            .map(|it| &it.id)
            .ok_or_else(|| ToolError::NoPackage {
                name: name.to_owned(),
            })
    }

    /// The directory `name` in `$BUNDLE_DIR` to assemble a bundle in, which is deleted first: never
    /// one that holds the workspace root or a package.
    pub fn bundle(&self, name: &str) -> Result<PathBuf, ToolError> {
        let bundles = PathBuf::from(variable(
            "BUNDLE_DIR",
            "the directory to assemble the bundle in",
        )?);
        let directory = self.root.join(bundles).join(name);

        if self.holds_sources(&directory) {
            Err(ToolError::BundleOverSources { path: directory })
        } else {
            Ok(directory)
        }
    }

    /// Whether `directory` is or holds the workspace root or a package, through `..` and
    /// symbolic links too.
    fn holds_sources(&self, directory: &Path) -> bool {
        let directory = resolved(directory);
        let packages = self
            .packages
            .iter()
            .filter_map(|it| it.manifest_path.parent())
            .map(Utf8Path::as_std_path);

        iter::once(self.root.as_path())
            .chain(packages)
            .any(|source| resolved(source).starts_with(&directory))
    }
}

/// `path` with its symbolic links and `..` resolved, though its end need not exist yet.
fn resolved(path: &Path) -> PathBuf {
    let Some((existing, real)) = path
        .ancestors()
        .find_map(|ancestor| Some((ancestor, ancestor.canonicalize().ok()?)))
    else {
        return path.to_owned();
    };
    let rest = path
        .strip_prefix(existing)
        .unwrap_or_else(|_| Path::new(""));

    rest.components().fold(real, |mut resolved, component| {
        match component {
            Component::ParentDir => {
                resolved.pop();
            }
            Component::Normal(name) => resolved.push(name),
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
        }

        resolved
    })
}

fn run_build(command: &mut Command) -> Result<Vec<u8>, ToolError> {
    output(command.arg("--message-format=json-render-diagnostics"))
}

/// Runs `command` and returns its standard output; its errors go to the terminal.
pub fn output(command: &mut Command) -> Result<Vec<u8>, ToolError> {
    let line = line(command);
    let output = command
        .stderr(Stdio::inherit())
        .output()
        .map_err(ToolError::start(line.clone()))?;

    exited(line, output.status)?;
    Ok(output.stdout)
}

/// Runs the Cargo build `command` and returns the `.<extension>` cdylib of `package_id`.
pub fn build_cdylib(
    command: &mut Command,
    package_id: &PackageId,
    extension: &str,
) -> Result<PathBuf, ToolError> {
    cdylib(&run_build(command)?, package_id, extension)
}

pub fn build_executable(
    command: &mut Command,
    package_id: &PackageId,
) -> Result<PathBuf, ToolError> {
    artifacts(&run_build(command)?, package_id)?
        .into_iter()
        .find_map(|it| it.executable)
        .map(PathBuf::from)
        .ok_or(ToolError::NoExecutable)
}

fn artifacts(messages: &[u8], package_id: &PackageId) -> Result<Vec<Artifact>, ToolError> {
    Message::parse_stream(messages)
        .filter_map(|message| match message {
            Ok(Message::CompilerArtifact(artifact)) if artifact.package_id == *package_id => {
                Some(Ok(artifact))
            }
            Ok(_) => None,
            Err(error) => Some(Err(ToolError::Messages(error))),
        })
        .collect()
}

fn cdylib(messages: &[u8], package_id: &PackageId, extension: &str) -> Result<PathBuf, ToolError> {
    artifacts(messages, package_id)?
        .into_iter()
        .filter(|it| it.target.is_cdylib())
        .flat_map(|it| it.filenames)
        .find(|path| path.extension() == Some(extension))
        .map(PathBuf::from)
        .ok_or(ToolError::NoCdylib)
}

pub fn cargo() -> Result<OsString, ToolError> {
    variable("CARGO", "the cargo that runs the xtask")
}

/// Set by the device shell.
pub fn device_target() -> Option<String> {
    env::var("ANAGRAM_TARGET").ok().filter(|it| !it.is_empty())
}

/// The environment variable `name`, `what` it names; an empty one counts as unset.
pub fn variable(name: &'static str, what: &'static str) -> Result<OsString, ToolError> {
    env::var_os(name)
        .filter(|it| !it.is_empty())
        .ok_or(ToolError::Unset { name, what })
}

pub fn tool(name: &'static str) -> Result<OsString, ToolError> {
    variable(name, "the tool to run")
}

pub fn directory(name: &'static str) -> Result<PathBuf, ToolError> {
    variable(name, "a directory").map(PathBuf::from)
}

pub fn line(command: &Command) -> String {
    iter::once(command.get_program())
        .chain(command.get_args())
        .map(OsStr::to_string_lossy)
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn run(command: &mut Command) -> Result<(), ToolError> {
    let line = line(command);
    let status = command.status().map_err(ToolError::start(line.clone()))?;

    exited(line, status)
}

/// Runs `source` with its standard output as the standard input of `sink`, then waits for both.
pub fn pipe(mut source: Command, mut sink: Command) -> Result<(), ToolError> {
    let source_line = line(&source);
    let sink_line = line(&sink);
    let mut source_child = source
        .stdout(Stdio::piped())
        .spawn()
        .map_err(ToolError::start(source_line.clone()))?;
    let output = source_child
        .stdout
        .take()
        .map_or_else(Stdio::null, Stdio::from);
    let sink_child = sink.stdin(output).spawn();

    drop(sink);

    let sink_status = sink_child.and_then(|mut it| it.wait());
    let source_status = source_child.wait();

    exited(
        sink_line.clone(),
        sink_status.map_err(ToolError::start(sink_line))?,
    )?;
    exited(
        source_line.clone(),
        source_status.map_err(ToolError::start(source_line))?,
    )
}

/// Runs the device binary `path` through the emulator on a build host.
pub fn emulated(path: &OsStr) -> Result<Command, ToolError> {
    let mut command = Command::new(variable("LV2_EMULATOR", "the emulator of device binaries")?);

    command.arg(path);
    Ok(command)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use cargo_metadata::PackageId;

    use super::{Command, ToolError, Workspace, cdylib, pipe};

    #[test]
    fn a_bundle_directory_over_the_root_or_a_package_holds_sources() {
        let workspace =
            Workspace::of(&Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
        let root = &workspace.root;

        assert!(workspace.holds_sources(root));
        assert!(workspace.holds_sources(&root.join(".")));
        assert!(workspace.holds_sources(&root.join("crates/crabgram")));
        assert!(workspace.holds_sources(&root.join("crates/missing/..")));
        assert!(workspace.holds_sources(&root.join("crates/crabgram-build/../..")));
        assert!(!workspace.holds_sources(&root.join("target/bundles/crabgram")));
        assert!(!workspace.holds_sources(&root.join("missing/../target")));
    }

    const MESSAGES: &str = concat!(
        r#"{"reason":"compiler-artifact","package_id":"path+file:///w/crates/crabgram#0.1.0","#,
        r#""target":{"name":"crabgram","kind":["lib"],"crate_types":["lib"],"src_path":"/w/lib.rs"},"#,
        r#""profile":{"opt_level":"3","debug_assertions":false,"overflow_checks":false,"test":false},"#,
        r#""features":[],"filenames":["/w/target/release/libcrabgram.rlib"],"executable":null,"fresh":true}"#,
        "\n",
        "a line that is no JSON\n",
        r#"{"reason":"compiler-artifact","package_id":"path+file:///w/examples/simple#0.1.0","#,
        r#""target":{"name":"simple","kind":["cdylib","rlib"],"crate_types":["cdylib","rlib"],"src_path":"/w/lib.rs"},"#,
        r#""profile":{"opt_level":"3","debug_assertions":false,"overflow_checks":false,"test":false},"#,
        r#""features":[],"filenames":["/w/target/release/libgain.rlib","/w/target/release/libgain.so"],"#,
        r#""executable":null,"fresh":false}"#,
        "\n",
        r#"{"reason":"build-finished","success":true}"#,
        "\n",
    );

    fn id(repr: &str) -> PackageId {
        PackageId {
            repr: repr.to_owned(),
        }
    }

    #[test]
    fn the_cdylib_is_the_shared_object_of_the_package() {
        let found = cdylib(
            MESSAGES.as_bytes(),
            &id("path+file:///w/examples/simple#0.1.0"),
            "so",
        );

        assert_eq!(
            found.unwrap(),
            std::path::Path::new("/w/target/release/libgain.so")
        );
    }

    #[test]
    fn a_package_without_a_cdylib_is_an_error() {
        assert!(
            cdylib(
                MESSAGES.as_bytes(),
                &id("path+file:///w/crates/crabgram#0.1.0"),
                "so"
            )
            .is_err()
        );
    }

    fn command(program: &str, arguments: &[&str]) -> Command {
        let mut command = Command::new(program);

        command.args(arguments);
        command
    }

    #[test]
    fn a_pipe_feeds_one_command_to_the_next_and_ends_when_either_does() {
        let archive = || command("echo", &["archive"]);
        let endless = || command("yes", &[]);

        assert!(
            pipe(
                archive(),
                command("sh", &["-c", r#"[ "$(cat)" = archive ]"#])
            )
            .is_ok()
        );
        assert!(matches!(
            pipe(archive(), command("false", &[])),
            Err(ToolError::Failed { ref line, .. }) if line == "false"
        ));
        assert!(matches!(
            pipe(endless(), command("true", &[])),
            Err(ToolError::Failed { ref line, .. }) if line == "yes"
        ));
        assert!(matches!(
            pipe(endless(), command("/missing/ssh", &[])),
            Err(ToolError::Start { ref line, .. }) if line == "/missing/ssh"
        ));
    }

    #[test]
    fn unreadable_messages_are_an_error() {
        assert!(cdylib(b"\xff\n", &id("path+file:///w/examples/simple#0.1.0"), "so").is_err());
    }
}
