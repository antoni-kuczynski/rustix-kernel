#!/bin/bash
set -e

nasm -felf64 ./boot/multiboot_header.asm -o boot/o/multiboot_header.o
nasm -felf64 ./boot/entry.asm -o boot/o/entry.o
nasm -felf64 ./boot/entry_efi.asm -o boot/o/entry_efi.o

cargo build --release

cp target/x86_64-rustix/release/rustix iso/boot/rustix

grub-mkrescue -o rustix.iso iso

cp /usr/share/ovmf/x64/OVMF_VARS.4m.fd ./iso/OVMF_VARS.4m.fd

qemu-system-x86_64 \
  -enable-kvm \
  -m 4G \
  -drive if=pflash,format=raw,readonly=on,file=/usr/share/ovmf/x64/OVMF_CODE.4m.fd \
  -drive if=pflash,format=raw,file=./iso/OVMF_VARS.4m.fd \
  -cdrom rustix.iso \
  -boot d \
  -device qemu-xhci \
  -no-reboot \
  -no-shutdown \
  -monitor stdio \
  -serial file:serial.log