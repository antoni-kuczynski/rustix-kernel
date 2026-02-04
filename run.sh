#!/bin/bash
set -e

nasm -felf64 ./boot/multiboot.asm -o boot/o/multiboot.o
nasm -felf64 ./boot/entry.asm -o boot/o/entry.o

cargo build --release

cp target/x86_64-rustix/release/rustix iso/boot/rustix

grub-mkrescue -o rustix.iso iso

qemu-system-x86_64 -cdrom rustix.iso -device qemu-xhci