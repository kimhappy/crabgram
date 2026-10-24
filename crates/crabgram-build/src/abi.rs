//! Checks that a device binary links only to the allowed libraries and glibc versions.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::tools::{self, ToolError, tool};

const DEVICE_LIBRARIES: [&str; 3] = ["libc.so.6", "libm.so.6", "ld-linux-aarch64.so.1"];

/// Why a device binary fails the ABI check, or could not be checked.
#[derive(Debug, thiserror::Error)]
pub enum AbiError {
    #[error(transparent)]
    Tool(#[from] ToolError),
    #[error("readelf rejected {}: {stderr}", binary.display())]
    Rejected { binary: PathBuf, stderr: String },
    #[error("the binary carries a runpath from the build host")]
    Runpath,
    #[error("the binary needs the unwinder (`_Unwind_*`), but a device build aborts on panic")]
    Unwinder,
    #[error("the binary needs {library}, which is not among the allowed libraries")]
    ForeignLibrary { library: String },
    #[error("DEVICE_GLIBC_VERSION {ceiling} is no version")]
    NoCeiling { ceiling: String },
    #[error("the binary needs GLIBC_{version}, which names no glibc release")]
    UnnamedGlibc { version: String },
    #[error("the binary needs GLIBC_{version}, newer than DEVICE_GLIBC_VERSION {ceiling}")]
    NewerGlibc { version: String, ceiling: String },
}

pub fn device_glibc() -> Result<String, ToolError> {
    tools::variable("DEVICE_GLIBC_VERSION", "the device's glibc")
        .map(|it| it.to_string_lossy().into_owned())
}

pub fn check(binary: &Path) -> Result<(), AbiError> {
    let ceiling = device_glibc()?;
    let mut readelf = Command::new(tool("READELF_TOOL")?);

    readelf
        .args(["--dynamic", "--dyn-syms", "--version-info", "--wide"])
        .arg(binary);

    let output = readelf
        .output()
        .map_err(ToolError::start(tools::line(&readelf)))?;

    if !output.status.success() {
        return Err(AbiError::Rejected {
            binary: binary.to_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    let elf = String::from_utf8_lossy(&output.stdout);

    if elf.contains("(RUNPATH)") || elf.contains("(RPATH)") {
        Err(AbiError::Runpath)
    } else if needs_unwinder(&elf) {
        Err(AbiError::Unwinder)
    } else {
        only_device_libraries(&elf)?;
        within_glibc(&elf, &ceiling)
    }
}

/// Whether a symbol table row of `elf` leaves an `_Unwind_*` symbol undefined, for a library to
/// provide.
fn needs_unwinder(elf: &str) -> bool {
    elf.lines().any(|row| {
        row.split_whitespace()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|pair| matches!(pair, ["UND", name] if name.starts_with("_Unwind_")))
    })
}

fn only_device_libraries(elf: &str) -> Result<(), AbiError> {
    elf.lines()
        .filter_map(|line| line.split_once("Shared library: ["))
        .map(|(_, library)| library.trim_end_matches(']'))
        .find(|library| !DEVICE_LIBRARIES.contains(library))
        .map_or(Ok(()), |library| {
            Err(AbiError::ForeignLibrary {
                library: library.to_owned(),
            })
        })
}

fn within_glibc(elf: &str, ceiling: &str) -> Result<(), AbiError> {
    let newest = numbers(ceiling).ok_or_else(|| AbiError::NoCeiling {
        ceiling: ceiling.to_owned(),
    })?;

    elf.split("GLIBC_")
        .skip(1)
        .map(|requirement| {
            requirement
                .split(|it: char| !(it.is_ascii_alphanumeric() || it == '_' || it == '.'))
                .next()
                .unwrap_or_default()
        })
        .filter(|version| *version != "PRIVATE")
        .try_for_each(|version| match numbers(version) {
            None => Err(AbiError::UnnamedGlibc {
                version: version.to_owned(),
            }),
            Some(needed) if needed > newest => Err(AbiError::NewerGlibc {
                version: version.to_owned(),
                ceiling: ceiling.to_owned(),
            }),
            Some(_) => Ok(()),
        })
}

fn numbers(version: &str) -> Option<Vec<u32>> {
    version
        .split('.')
        .map(str::parse::<u32>)
        .map(Result::ok)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DYNAMIC: &str = "
Dynamic section at offset 0x1fd40 contains 24 entries:
  Tag        Type                         Name/Value
 0x0000000000000001 (NEEDED)             Shared library: [libc.so.6]
 0x0000000000000001 (NEEDED)             Shared library: [ld-linux-aarch64.so.1]
";

    fn symbols(version: &str) -> String {
        format!(
            "
Symbol table '.dynsym' contains 3 entries:
   Num:    Value          Size Type    Bind   Vis      Ndx Name
     1: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND memcpy@GLIBC_2.17 (2)
     2: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND expf@{version} (3)

Version needs section '.gnu.version_r' contains 1 entry:
  0x0010: Name: GLIBC_2.17  Flags: none  Version: 2
  0x0020: Name: {version}  Flags: none  Version: 3
"
        )
    }

    #[test]
    fn a_symbol_newer_than_the_device_glibc_fails() {
        assert!(within_glibc(&symbols("GLIBC_2.35"), "2.34").is_err());
        assert!(within_glibc(&symbols("GLIBC_2.34"), "2.34").is_ok());
    }

    #[test]
    fn a_point_release_is_newer_than_its_minor() {
        assert!(within_glibc(&symbols("GLIBC_2.34.1"), "2.34").is_err());
        assert!(within_glibc(&symbols("GLIBC_2.9"), "2.34").is_ok());
    }

    #[test]
    fn glibc_private_carries_no_version() {
        assert!(within_glibc(&symbols("GLIBC_PRIVATE"), "2.34").is_ok());
    }

    #[test]
    fn a_named_glibc_requirement_fails() {
        assert!(within_glibc(&symbols("GLIBC_ABI_DT_RELR"), "2.34").is_err());
        assert!(within_glibc(&symbols("GLIBC_ABI_GNU2_TLS"), "2.34").is_err());
    }

    #[test]
    fn only_an_undefined_unwinder_symbol_needs_libgcc_s() {
        let table = |row: &str| format!("{}{row}\n", symbols("GLIBC_2.17"));

        assert!(!needs_unwinder(&symbols("GLIBC_2.17")));
        assert!(needs_unwinder(&table(
            "     3: 0000000000000000     0 FUNC    GLOBAL DEFAULT  UND _Unwind_Resume@GCC_3.0 (4)"
        )));
        assert!(!needs_unwinder(&table(
            "     3: 0000000000012340    64 FUNC    GLOBAL DEFAULT   12 _Unwind_Resume"
        )));
        assert!(!needs_unwinder(
            "  0x0030: Name: _Unwind_Resume  Flags: none  Version: 4\n"
        ));
    }

    #[test]
    fn only_the_device_libraries_are_needed() {
        let unwinder = format!(
            "{DYNAMIC} 0x0000000000000001 (NEEDED)             Shared library: [libgcc_s.so.1]\n"
        );

        assert!(only_device_libraries(DYNAMIC).is_ok());
        assert!(only_device_libraries(&unwinder).is_err());
    }
}
