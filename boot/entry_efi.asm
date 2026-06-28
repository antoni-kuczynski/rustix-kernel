section .text._start progbits alloc exec nowrite align=16

global _start_efi_amd64

extern _start
extern rust_main


MULTIBOOT2_BOOTLOADER_MAGIC equ 0x36D76289
MULTIBOOT_TAG_TYPE_EFI64    equ 12
EFI_SYSTEM_TABLE_CONOUT     equ 64
EFI_TEXT_OUTPUT_STRING      equ 8

_start_efi_amd64:
    call rust_main
