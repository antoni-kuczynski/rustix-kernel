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

if [[ ! -x "${GRUB_MKSTANDALONE}" ]]; then
  GRUB_MKSTANDALONE="grub-mkstandalone"
  GRUB_MODULE_DIR="/usr/lib/grub/x86_64-efi"
fi

mkdir -p boot/o
mkdir -p "${UEFI_DIR}/boot/grub"

nasm -felf64 ./boot/multiboot_header.asm -o boot/o/multiboot_header.o
nasm -felf64 ./boot/entry.asm -o boot/o/entry.o
nasm -felf64 ./boot/entry_efi.asm -o boot/o/entry_efi.o

rm -f target/x86_64-rustix/release/rustix target/x86_64-rustix/release/deps/rustix-*

cargo build --release

rm -rf "${ISO_ROOT}"
mkdir -p "${ISO_ROOT}/boot/grub"
cp "${KERNEL}" "${ISO_ROOT}/boot/rustix"

cat > "${GRUB_CFG}" <<'EOF'
set timeout=0
set default=0

set root=(memdisk)
insmod all_video
set gfxpayload=1024x768x32,auto

multiboot2 /boot/rustix
boot
EOF

cp "${GRUB_CFG}" "${UEFI_DIR}/boot/grub/grub.cfg"

echo "Using $(${GRUB_MKSTANDALONE} --version)"

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

rm -f "${EFI_IMG}"
mformat -C -T 32768 -i "${EFI_IMG}" ::
mmd -i "${EFI_IMG}" ::/EFI ::/EFI/BOOT
mcopy -i "${EFI_IMG}" "${GRUB_EFI}" ::/EFI/BOOT/BOOTX64.EFI
mkdir -p "${ISO_ROOT}/EFI/BOOT"
cp "${GRUB_EFI}" "${ISO_ROOT}/EFI/BOOT/BOOTX64.EFI"

xorriso \
  -as mkisofs \
  -R -J \
  -e efi.img \
  -no-emul-boot \
  -o "${ISO_OUT}" \
  "${ISO_ROOT}"

echo "Starting QEMU with GDB..."

qemu-system-x86_64 \
  -cpu qemu64 \
  -m 512M \
  -machine q35 \
  -vga std \
  -drive if=pflash,format=raw,readonly=on,file="${OVMF_CODE}" \
  -drive if=pflash,format=raw,file="${OVMF_VARS}" \
  -cdrom "${ISO_OUT}" \
  -boot d \
  -D log.txt \
  -device qemu-xhci \
  -serial file:serial.log \
  -s -S &

QEMU_PID=$!

trap "kill -9 $QEMU_PID 2>/dev/null" EXIT

sleep 1

gdb -tui \
    -ex "target remote localhost:1234" \
    -ex "layout src" \
    "${KERNEL}"