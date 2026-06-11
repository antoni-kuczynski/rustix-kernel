#!/bin/bash
set -e

KERNEL_NAME="rustix"
TARGET="x86_64-rustix"
PROFILE="debug"

KERNEL_ELF="target/${TARGET}/${PROFILE}/${KERNEL_NAME}"
ISO_KERNEL="iso/boot/${KERNEL_NAME}"
ISO_FILE="rustix.iso"
GDB_PORT="1234"

mkdir -p boot/o

nasm -felf64 ./boot/multiboot_header.asm -o boot/o/multiboot_header.o
nasm -felf64 ./boot/entry.asm -o boot/o/entry.o

cargo build

cp "${KERNEL_ELF}" "${ISO_KERNEL}"

grub-mkrescue -o "${ISO_FILE}" iso

cat > /tmp/rustix-gdb.gdb <<EOF
set architecture i386:x86-64
set disassembly-flavor intel
set pagination off
target remote :${GDB_PORT}

layout split
layout regs

define pf
    echo \\n--- PAGE FAULT DEBUG ---\\n
    info registers
    echo \\nCR2:\\n
    p/x \$cr2
    echo \\nCR3:\\n
    p/x \$cr3
    echo \\nRIP instruction:\\n
    x/8i \$rip
    echo \\nStack:\\n
    x/16gx \$rsp
end
EOF

qemu-system-x86_64 \
    -cdrom "${ISO_FILE}" \
    -device qemu-xhci \
    -m 128 \
    -drive if=pflash,format=raw,readonly=on,file=/usr/share/ovmf/x64/OVMF_CODE.4m.fd \
    -drive if=pflash,format=raw,file=./iso/OVMF_VARS.4m.fd \
    -d int,cpu_reset,guest_errors \
    -no-reboot \
    -no-shutdown \
    -D log.txt \
    -S \
    -s &

QEMU_PID=$!

cleanup() {
    kill "${QEMU_PID}" 2>/dev/null || true
}

trap cleanup EXIT

if command -v rust-gdb >/dev/null 2>&1; then
    rust-gdb -tui "${KERNEL_ELF}" -x /tmp/rustix-gdb.gdb
else
    gdb -tui "${KERNEL_ELF}" -x /tmp/rustix-gdb.gdb
fi