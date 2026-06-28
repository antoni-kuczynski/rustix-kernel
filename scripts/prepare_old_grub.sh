#!/usr/bin/env bash
# This script installs old grub 2.12 locally to run the kernel with it.
# On newest version 2.14 there's a problem in UEFI boot:
# Grub tries to patch it's small memory stub before entering the kernel, but the memory page
# it's trying to modify is set to read-only, resulting in a page fault.
# All of that happens before jump to kernel code is even made.
# This may be a bug in Grub, so if I won't find anything new here, I'll just make an issue/PR on grub's repo and move on...
# For now, I'm sick of debugging this, so let downgrading be the solution :)

set -euo pipefail

GRUB_ARCHIVE_URL="${GRUB_ARCHIVE_URL:-https://archive.archlinux.org/packages/g/grub/grub-2%3A2.12-3-x86_64.pkg.tar.zst}"
GRUB_OLD_DIR="${GRUB_OLD_DIR:-target/grub-old}"
GRUB_ROOT="${GRUB_ROOT:-${GRUB_OLD_DIR}/root}"
GRUB_PKG_DIR="${GRUB_PKG_DIR:-${GRUB_OLD_DIR}/pkg}"
GRUB_PKG="${GRUB_PKG:-${GRUB_PKG_DIR}/grub-2.12-3-x86_64.pkg.tar.zst}"
FORCE="${FORCE:-0}"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    return 1
  fi
}

missing=0
for cmd in curl tar nasm cargo mformat mmd mcopy xorriso objcopy qemu-system-x86_64; do
  if ! need_cmd "${cmd}"; then
    missing=1
  fi
done

if [[ "${missing}" -ne 0 ]]; then
  echo
  echo "Install missing tools first. On Arch this is usually something like:"
  echo "  sudo pacman -S curl tar nasm rust mtools libisoburn binutils qemu-system-x86"
  exit 1
fi

mkdir -p "${GRUB_PKG_DIR}" "${GRUB_ROOT}"

if [[ "${FORCE}" == "1" ]]; then
  rm -rf "${GRUB_ROOT}"
  mkdir -p "${GRUB_ROOT}"
fi

if [[ ! -f "${GRUB_PKG}" ]]; then
  echo "Downloading GRUB 2.12-3 package:"
  echo "  ${GRUB_ARCHIVE_URL}"
  curl -fL "${GRUB_ARCHIVE_URL}" -o "${GRUB_PKG}"
else
  echo "Using cached package:"
  echo "  ${GRUB_PKG}"
fi

if [[ ! -x "${GRUB_ROOT}/usr/bin/grub-mkstandalone" ]]; then
  echo "Extracting package to:"
  echo "  ${GRUB_ROOT}"
  tar -C "${GRUB_ROOT}" -xf "${GRUB_PKG}"
fi

if [[ ! -x "${GRUB_ROOT}/usr/bin/grub-mkstandalone" ]]; then
  echo "grub-mkstandalone was not found after extraction" >&2
  exit 1
fi

if [[ ! -f "${GRUB_ROOT}/usr/lib/grub/x86_64-efi/multiboot2.mod" ]]; then
  echo "x86_64-efi multiboot2.mod was not found after extraction" >&2
  exit 1
fi

echo
echo "Prepared local GRUB:"
"${GRUB_ROOT}/usr/bin/grub-mkstandalone" --version
echo
echo "run_uefi will use:"
echo "  GRUB_ROOT=${GRUB_ROOT}"
echo "  GRUB_MKSTANDALONE=${GRUB_ROOT}/usr/bin/grub-mkstandalone"
echo "  GRUB_MODULE_DIR=${GRUB_ROOT}/usr/lib/grub/x86_64-efi"
echo
echo "Next:"
echo "  ./run_uefi"

