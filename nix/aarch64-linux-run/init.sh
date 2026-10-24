#!/bin/busybox sh

/bin/busybox --install -s /bin
export PATH=/bin

mkdir -p /proc /sys /dev /tmp
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t devtmpfs devtmpfs /dev

share() {
    mkdir -p "$3" && mount -t 9p -o "trans=virtio,version=9p2000.L,$1" "$2" "$3"
}

share ro store /nix/store
share rw work /run/work

while read -r tag path; do
    share ro "$tag" "$path"
done < /run/work/mounts

@bash@ /run/work/command > /run/work/stdout 2> /run/work/stderr
echo $? > /run/work/status

sync
poweroff -f
