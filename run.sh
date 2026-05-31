#!/bin/bash
set -e

nasm -felf64 ./boot/multiboot.asm -o boot/o/multiboot.o
nasm -felf64 ./boot/entry.asm -o boot/o/entry.o

cargo build --release -Zjson-target-spec

cp target/x86_64-rustix/release/rustix iso/boot/rustix

grub-mkrescue -o rustix.iso iso

qemu-system-x86_64 \
  -cdrom rustix.iso \
  -m 8G \
  -device qemu-xhci,id=xhci \
  -device usb-ehci,id=ehci \
  -device pci-ohci,id=ohci \
  -device piix3-usb-uhci,id=uhci \
  -d int,cpu_reset \
  -no-reboot \
  -D log.txt