#!/bin/bash
set -euo pipefail
# Uses old grub for booting - see prepare_old_grub for info

UEFI_DIR="uefi"
ISO_ROOT="target/uefi-iso-root"
ISO_OUT="uefi.iso"
EFI_IMG="${ISO_ROOT}/efi.img"
GRUB_CFG="${ISO_ROOT}/grub.cfg"
GRUB_EFI="${ISO_ROOT}/BOOTX64.EFI"
KERNEL="target/x86_64-rustix/release/rustix"
OVMF_CODE="ovmf/OVMF_CODE.fd"
OVMF_VARS="ovmf/OVMF_VARS.fd"
GRUB_ROOT="${GRUB_ROOT:-target/grub-old/root}"
GRUB_MKSTANDALONE="${GRUB_MKSTANDALONE:-${GRUB_ROOT}/usr/bin/grub-mkstandalone}"
GRUB_MODULE_DIR="${GRUB_MODULE_DIR:-${GRUB_ROOT}/usr/lib/grub/x86_64-efi}"

MODE="uefi"
GDB_FLAGS=""
USB_DISK=""
BUILD_MODE="debug"
CARGO_FLAGS=""

print_help() {
  echo "Usage: $0 [OPTIONS]"
  echo "Build hybrid ISO image and boots it in qemu."
  echo ""
  echo "Options:"
  echo "  -h, --help        Prints this message."
  echo "  --bios            Boots qemu in legacy BIOS mode."
  echo "                    Without this option qemu will boot in UEFI mode."
  echo "  --gdb             Starts GDB server on port 1234 and launches GDB tui."
  echo "  --release         Builds and runs in release mode (default is dev)."
  echo "  --usb /dev/sdX    Flashes the generated Hybrid ISO to the specified USB drive."
  echo "                    (Skips QEMU execution)."
  echo ""
  echo "Note: No matter which options are specified, the built ISO will always"
  echo "contain both BIOS and UEFI boot files (hybrid)."
  echo ""
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      print_help
      exit 0
      ;;
    --bios)
      MODE="bios"
      shift
      ;;
    --gdb)
      GDB_FLAGS="-s -S"
      shift
      ;;
    --release)
      BUILD_MODE="release"
      CARGO_FLAGS="--release"
      shift
      ;;
    --usb)
      if [[ -z "${2:-}" ]]; then
        echo "Error: --usb requires a device path (e.g., /dev/sdb)"
        exit 1
      fi
      USB_DISK="$2"
      shift 2
      ;;
    *)
      echo "Invalid argument '$1'"
      echo ""
      print_help
      exit 1
      ;;
  esac
done

KERNEL="target/x86_64-rustix/${BUILD_MODE}/rustix"

if [[ ! -x "${GRUB_MKSTANDALONE}" ]]; then
  GRUB_MKSTANDALONE="grub-mkstandalone"
  GRUB_MODULE_DIR="/usr/lib/grub/x86_64-efi"
fi

echo "==> Compiling Assembly and Rust Code"
mkdir -p boot/o
mkdir -p "${UEFI_DIR}/boot/grub"

nasm -felf64 ./boot/multiboot_header.asm -o boot/o/multiboot_header.o
nasm -felf64 ./boot/entry.asm -o boot/o/entry.o
nasm -felf64 ./boot/entry_efi.asm -o boot/o/entry_efi.o

rm -f "${KERNEL}" "target/x86_64-rustix/${BUILD_MODE}/deps/rustix-*"
cargo build ${CARGO_FLAGS}

echo "==> Preparing ISO Directory"
rm -rf "${ISO_ROOT}"
mkdir -p "${ISO_ROOT}/boot/grub"
cp "${KERNEL}" "${ISO_ROOT}/boot/rustix"


# ======================================================================================================================
# 1. BUILDING UEFI PAYLOAD
# ======================================================================================================================
echo "==> Building UEFI payload using $(${GRUB_MKSTANDALONE} --version)"

cat > "${GRUB_CFG}" <<'EOF'
set timeout=0
set default=0

set root=(memdisk)
insmod all_video
set gfxpayload=keep

multiboot2 /boot/rustix
boot
EOF

cp "${GRUB_CFG}" "${UEFI_DIR}/boot/grub/grub.cfg"

"${GRUB_MKSTANDALONE}" \
  -O x86_64-efi \
  -d "${GRUB_MODULE_DIR}" \
  -o "${GRUB_EFI}" \
  --locales='' \
  --fonts='' \
  --themes='' \
  --install-modules="multiboot2 boot all_video efi_gop efi_uga video video_fb normal configfile" \
  "/boot/grub/grub.cfg=${GRUB_CFG}" \
  "/boot/rustix=${KERNEL}"

objcopy \
  --set-section-flags .text=alloc,load,code,data \
  --set-section-flags mods=alloc,load,code,data \
  "${GRUB_EFI}"

# Virtual EFI FAT32 partition
rm -f "${EFI_IMG}"
mformat -C -T 32768 -i "${EFI_IMG}" ::
mmd -i "${EFI_IMG}" ::/EFI ::/EFI/BOOT
mcopy -i "${EFI_IMG}" "${GRUB_EFI}" ::/EFI/BOOT/BOOTX64.EFI
mkdir -p "${ISO_ROOT}/EFI/BOOT"
cp "${GRUB_EFI}" "${ISO_ROOT}/EFI/BOOT/BOOTX64.EFI"


# ======================================================================================================================
# 2. BUILDING LEGACY BIOS PAYLOAD
# ======================================================================================================================
GRUB_MODULE_DIR_BIOS="${GRUB_ROOT}/usr/lib/grub/i386-pc"
if [[ ! -d "${GRUB_MODULE_DIR_BIOS}" ]]; then
  GRUB_MODULE_DIR_BIOS="/usr/lib/grub/i386-pc"
fi

if [[ -d "${GRUB_MODULE_DIR_BIOS}" ]]; then
  echo "==> Building Legacy BIOS payload (i386-pc)..."

# Dedicated config for BIOS
GRUB_BIOS_CFG="${ISO_ROOT}/grub_bios.cfg"
cat > "${GRUB_BIOS_CFG}" <<'EOF'
set timeout=0
set default=0

# Needed so that grub can find kernel image
insmod biosdisk

insmod all_video
insmod iso9660
insmod part_msdos
insmod part_gpt
insmod search
insmod search_fs_file
set gfxpayload=keep

search --file --no-floppy --set=root /boot/rustix

if [ -f /boot/rustix ]; then
    multiboot2 /boot/rustix
    boot
fi
EOF

  mkdir -p "${ISO_ROOT}/boot/grub/i386-pc"
  BIOS_CORE="${ISO_ROOT}/boot/grub/i386-pc/core.img"
  BIOS_ELTORITO="${ISO_ROOT}/boot/grub/i386-pc/eltorito.img"

  "${GRUB_MKSTANDALONE}" \
    -O i386-pc \
    -d "${GRUB_MODULE_DIR_BIOS}" \
    -o "${BIOS_CORE}" \
    --locales='' \
    --fonts='' \
    --themes='' \
    --install-modules="multiboot2 boot all_video normal configfile biosdisk iso9660 part_msdos part_gpt search search_fs_file test echo ls sleep" \
    "/boot/grub/grub.cfg=${GRUB_BIOS_CFG}"

  cat "${GRUB_MODULE_DIR_BIOS}/cdboot.img" "${BIOS_CORE}" > "${BIOS_ELTORITO}"
else
  echo "==> WARNING: grub i386-pc modules not found! Legacy boot will fail."
fi


# ======================================================================================================================
# 3. COMBINING ALL OF THAT INTO HYBRIS ISO
# ======================================================================================================================
echo "==> Generating Hybrid ISO..."
xorriso \
  -as mkisofs \
  -R -J \
  -V "RUSTIX" \
  -boot-info-table \
  -b boot/grub/i386-pc/eltorito.img \
  -no-emul-boot \
  -boot-load-size 4 \
  --grub2-boot-info \
  --grub2-mbr "${GRUB_MODULE_DIR_BIOS}/boot_hybrid.img" \
  --mbr-force-bootable \
  -partition_offset 16 \
  -eltorito-alt-boot \
  -e efi.img \
  -no-emul-boot \
  -isohybrid-gpt-basdat \
  -o "${ISO_OUT}" \
  "${ISO_ROOT}"

# ======================================================================================================================
# 4. USB BURNING LOGIC
# ======================================================================================================================
die() { echo "error: $*" >&2; exit 1; }
status() { echo "==> $*"; }

if [[ -n "${USB_DISK}" ]]; then
  [[ -b "${USB_DISK}" ]] || die "${USB_DISK} is not a block device"
  [[ -f "${ISO_OUT}" ]] || die "${ISO_OUT} not found"

  if [[ "${USB_DISK}" =~ [0-9]$ && ! "${USB_DISK}" =~ nvme[0-9]+n[0-9]+$ && ! "${USB_DISK}" =~ mmcblk[0-9]+$ ]]; then
    die "pass the whole disk, not a partition (e.g. /dev/sdX)"
  fi

  status "Selected target device"
  lsblk "${USB_DISK}" -o NAME,SIZE,MODEL,TYPE,FSTYPE,MOUNTPOINTS

  echo
  read -rp "This will COMPLETELY ERASE ${USB_DISK}. Type the device path to continue: " CONFIRM
  [[ "$CONFIRM" == "${USB_DISK}" ]] || die "cancelled by user"

  status "Requesting sudo privileges for disk operations..."
  sudo -v

  status "Unmounting target partitions..."
  while read -r mp; do
    [[ -n "$mp" ]] && sudo umount "$mp" || true
  done < <(lsblk -nrpo MOUNTPOINTS "${USB_DISK}" | grep -v '^$' || true)

  status "Flashing Hybrid ISO to ${USB_DISK} (This may take a moment)..."
  sudo dd if="${ISO_OUT}" of="${USB_DISK}" bs=4M status=progress
  sudo sync

  status "Successfully flashed ${USB_DISK}!"
  exit 0 # Dont boot qemu if using this mode
fi


# ======================================================================================================================
# 5. BOOTING IN QEMU
# ======================================================================================================================
run_qemu() {
  if [[ -n "${GDB_FLAGS}" ]]; then
    "$@" &
  else
    "$@"
  fi
}


if [[ "${MODE}" == "bios" ]]; then
  echo "==> Booting Legacy BIOS..."
  run_qemu qemu-system-x86_64 \
    -m 4G \
    -machine pc \
    -vga std \
    -cdrom "${ISO_OUT}" \
    -boot d \
    -D log.txt \
    -device qemu-xhci,id=xhci \
    -device usb-ehci,id=ehci \
    -device pci-ohci,id=ohci \
    -device piix3-usb-uhci,id=uhci \
    -monitor stdio \
    -serial file:serial.log \
    $GDB_FLAGS
else
  echo "==> Booting UEFI..."
  run_qemu qemu-system-x86_64 \
    -m 4G \
    -machine q35 \
    -vga std \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
    -drive if=pflash,format=raw,file="${OVMF_VARS}" \
    -cdrom "${ISO_OUT}" \
    -boot d \
    -D log.txt \
    -device qemu-xhci,id=xhci \
    -device usb-ehci,id=ehci \
    -device pci-ohci,id=ohci \
    -device piix3-usb-uhci,id=uhci \
    -monitor stdio \
    -serial file:serial.log \
    $GDB_FLAGS
fi

if [[ -n "${GDB_FLAGS}" ]]; then
    QEMU_PID=$!

    trap "kill -9 $QEMU_PID 2>/dev/null" EXIT

    sleep 1

    gdb -tui \
        -ex "target remote localhost:1234" \
        -ex "layout src" \
        "${KERNEL}"
fi

if [[ -n "${GDB_FLAGS}" ]]; then
    QEMU_PID=$!

    trap "kill -9 $QEMU_PID 2>/dev/null" EXIT

    sleep 1

    gdb -tui \
        -ex "target remote localhost:1234" \
        -ex "layout src" \
        "${KERNEL}"
fi
