//! Installs the bundle on a host over one SSH connection.

use std::{path::Path, process::Command};

use crate::{
    Error, Project, Target, check, info,
    tools::{self, Workspace},
};

pub const DEVICE_HOST: &str = "root@192.168.51.1";
const DEVICE_LV2_DIRECTORY: &str = "/root/.lv2";
/// Restarted after the install.
const DEVICE_SERVICES: [&str; 2] = ["jack2", "lvgl-app"];

/// `host`, unless ssh would take it for no destination or for an option.
pub fn destination(host: &str) -> Result<String, Error> {
    if host.is_empty() || host.starts_with('-') {
        Err(Error::Host {
            host: host.to_owned(),
        })
    } else {
        Ok(host.to_owned())
    }
}

pub fn deploy<P: Project>(host: &str) -> Result<(), Error> {
    let target @ Target::Device { .. } = Target::current() else {
        return Err(Error::NativeDeploy);
    };
    let directory = check::checked::<P>(&Workspace::load()?, &target)?;
    let [archive, install] = commands(host, &directory, &info::<P>().bundle());

    tools::pipe(archive, install)?;
    println!("deployed {} to {host}", directory.display());
    Ok(())
}

/// `text` as one word of the shell.
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// Unpacks the archive on its input beside the installed bundle, swaps the two and restarts
/// [`DEVICE_SERVICES`].
fn script(bundle: &str) -> String {
    let bundle = format!("{DEVICE_LV2_DIRECTORY}/{bundle}");
    let lv2 = quoted(DEVICE_LV2_DIRECTORY);
    let installed = quoted(&bundle);
    let staged = quoted(&format!("{bundle}.staged"));
    let manifest = quoted(&format!("{bundle}.staged/{}", crate::turtle::MANIFEST));
    let old = quoted(&format!("{bundle}.old"));

    [
        "set -e".to_owned(),
        format!("mkdir -p {lv2}"),
        format!("rm -rf {staged} {old}"),
        format!("mkdir {staged}"),
        format!("tar -x -o -f - -C {staged}"),
        format!("test -e {manifest}"),
        format!("if [ -e {installed} ]; then mv {installed} {old}; fi"),
        format!("mv {staged} {installed}"),
        format!("rm -rf {old}"),
        format!("systemctl restart {}", DEVICE_SERVICES.join(" ")),
    ]
    .join("; ")
}

/// The archive of the bundle at `directory`, and the SSH command that installs it on `host`.
fn commands(host: &str, directory: &Path, bundle: &str) -> [Command; 2] {
    let mut archive = Command::new("tar");
    let mut install = Command::new("ssh");

    archive
        .env("COPYFILE_DISABLE", "1")
        .arg("-C")
        .arg(directory)
        .args(["-c", "-f", "-", "."]);
    install
        .args(["--", host])
        .arg(format!("sh -c {}", quoted(&script(bundle))));
    [archive, install]
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{commands, quoted, script};
    use crate::tools::line;

    #[test]
    fn a_deploy_unpacks_the_bundle_beside_the_installed_one_swaps_them_and_restarts_the_services() {
        assert_eq!(
            script("gain.lv2"),
            "set -e; mkdir -p '/root/.lv2'; \
             rm -rf '/root/.lv2/gain.lv2.staged' '/root/.lv2/gain.lv2.old'; \
             mkdir '/root/.lv2/gain.lv2.staged'; tar -x -o -f - -C '/root/.lv2/gain.lv2.staged'; \
             test -e '/root/.lv2/gain.lv2.staged/manifest.ttl'; \
             if [ -e '/root/.lv2/gain.lv2' ]; then mv '/root/.lv2/gain.lv2' '/root/.lv2/gain.lv2.old'; fi; \
             mv '/root/.lv2/gain.lv2.staged' '/root/.lv2/gain.lv2'; rm -rf '/root/.lv2/gain.lv2.old'; \
             systemctl restart jack2 lvgl-app"
        );
    }

    #[test]
    fn the_bundle_goes_over_one_connection_as_an_archive() {
        let [archive, install] = commands("root@anagram", Path::new("/build/gain.lv2"), "gain.lv2");
        let lines = [&archive, &install].map(line);

        assert_eq!(
            archive.get_envs().collect::<Vec<_>>(),
            [("COPYFILE_DISABLE".as_ref(), Some("1".as_ref()))],
            "the archive holds no macOS metadata entries"
        );

        assert_eq!(
            lines,
            [
                "tar -C /build/gain.lv2 -c -f - .".to_owned(),
                format!("ssh -- root@anagram sh -c {}", quoted(&script("gain.lv2"))),
            ]
        );
        assert_eq!(quoted("it's"), r"'it'\''s'");
    }
}
