#!/usr/bin/env bash
set -euo pipefail

ISO="${2:-uefi.iso}"
DISK="${1:-}"

die() {
  echo "error: $*" >&2
  exit 1
}

status() {
  echo "==> $*"
}

[[ "$EUID" -eq 0 ]] || die "run as root"
[[ -n "$DISK" ]] || die "usage: sudo $0 /dev/sdX [iso]"
[[ -b "$DISK" ]] || die "$DISK is not a block device"
[[ -f "$ISO" ]] || die "$ISO not found"

if [[ "$DISK" =~ [0-9]$ && ! "$DISK" =~ nvme[0-9]+n[0-9]+$ && ! "$DISK" =~ mmcblk[0-9]+$ ]]; then
  die "pass the whole disk, not a partition"
fi

for cmd in lsblk parted mkfs.vfat mount umount rsync findmnt dd sync; do
  command -v "$cmd" >/dev/null 2>&1 || die "missing command: $cmd"
done

status "Selected target device"
lsblk "$DISK" -o NAME,SIZE,MODEL,TYPE,FSTYPE,MOUNTPOINTS

echo
read -rp "This will erase $DISK. Type the device path to continue: " CONFIRM
[[ "$CONFIRM" == "$DISK" ]] || die "cancelled"

status "Unmounting target partitions"
while read -r mp; do
  [[ -n "$mp" ]] && umount "$mp" || true
done < <(lsblk -nrpo MOUNTPOINTS "$DISK" | grep -v '^$' || true)

status "Wiping partition table"
dd if=/dev/zero of="$DISK" bs=1M count=16 status=none
sync

status "Creating EFI system partition"
parted -s "$DISK" mklabel gpt
parted -s "$DISK" mkpart ESP fat32 1MiB 100%
parted -s "$DISK" set 1 esp on
partprobe "$DISK" || true
sleep 1

if [[ "$DISK" =~ ^/dev/nvme|^/dev/mmcblk|^/dev/loop ]]; then
  PART="${DISK}p1"
else
  PART="${DISK}1"
fi

[[ -b "$PART" ]] || die "$PART was not created"

status "Formatting EFI partition"
mkfs.vfat -F32 -n RUSTIX "$PART" >/dev/null

TMP_ISO="$(mktemp -d)"
TMP_USB="$(mktemp -d)"

cleanup() {
  set +e
  mountpoint -q "$TMP_ISO" && umount "$TMP_ISO"
  mountpoint -q "$TMP_USB" && umount "$TMP_USB"
  rmdir "$TMP_ISO" "$TMP_USB" 2>/dev/null
}
trap cleanup EXIT

status "Mounting ISO and USB"
mount -o loop,ro "$ISO" "$TMP_ISO"
mount "$PART" "$TMP_USB"

status "Copying ISO contents"
rsync -aH --info=progress2 "$TMP_ISO"/ "$TMP_USB"/

[[ -f "$TMP_USB/EFI/BOOT/BOOTX64.EFI" ]] || die "EFI/BOOT/BOOTX64.EFI not found on USB"

status "Syncing data"
sync

status "Done"