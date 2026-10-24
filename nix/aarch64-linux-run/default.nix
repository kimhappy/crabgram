{
  runCommand,
  writeShellApplication,
  replaceVars,
  cpio,
  coreutils,
  gawk,
  qemu_kvm,
  linuxPkgs,
}:
let
  initrd = runCommand "aarch64-linux-run-initrd" { nativeBuildInputs = [ cpio ]; } ''
    mkdir -p root/bin
    cp ${linuxPkgs.pkgsStatic.busybox}/bin/busybox root/bin/
    install -m 755 ${replaceVars ./init.sh { bash = "${linuxPkgs.bash}/bin/bash"; }} root/init
    (cd root && find . | sort | cpio --quiet -o -H newc) > $out
  '';
in
writeShellApplication {
  name = "aarch64-linux-run";
  runtimeInputs = [
    coreutils
    gawk
    qemu_kvm
  ];
  text = ''
    work=$(mktemp -d "''${TMPDIR:-/tmp}/aarch64-linux-run.XXXXXX")
    trap 'rm -rf "$work"' EXIT

    is_host() { [[ $1 == /* && $1 != /nix/store/* && -e $1 ]]; }

    IFS=: read -ra lv2_path <<< "''${LV2_PATH:-}"

    directories=()
    for path in "$@" "''${lv2_path[@]}"; do
      is_host "$path" || continue
      [[ -d $path ]] || path=''${path%/*}
      directories+=("$path")
    done

    mapfile -t shared < <(
      printf '%s\n' "''${directories[@]}" | sort -u |
        awk 'length($0) && (top == "" || index($0, top "/") != 1) { print; top = $0 }'
    )

    {
      printf 'export LV2_PATH=%q\n' "''${LV2_PATH:-}"
      printf 'exec'
      printf ' %q' "$@"
      printf '\n'
    } > "$work/command"

    virtfs() {
      printf -- '-virtfs\0local,path=%s,mount_tag=%s,security_model=none%s\0' \
        "''${1//,/,,}" "$2" "$3"
    }

    mapfile -d "" -t devices < <(
      virtfs /nix/store store ,readonly=on
      virtfs "$work" work ""
      for index in "''${!shared[@]}"; do
        virtfs "''${shared[$index]}" "share$index" ,readonly=on
      done
    )

    for index in "''${!shared[@]}"; do
      printf 'share%s %s\n' "$index" "''${shared[$index]}"
    done > "$work/mounts"

    qemu-system-aarch64 \
      -machine virt -accel hvf -accel tcg -cpu max -smp 2 -m 1024 \
      -nographic -no-reboot \
      -kernel ${linuxPkgs.linuxPackages.kernel}/Image \
      -initrd ${initrd} \
      -append "console=ttyAMA0 quiet loglevel=1 panic=-1" \
      "''${devices[@]}" \
      < /dev/null > "$work/console" 2>&1 || true

    cat "$work/stdout" 2> /dev/null || true
    cat "$work/stderr" >&2 2> /dev/null || true

    [[ -f $work/status ]] || {
      cat "$work/console" >&2
      echo "aarch64-linux-run: the VM stopped before $1 finished" >&2
      exit 1
    }

    exit "$(< "$work/status")"
  '';
}
