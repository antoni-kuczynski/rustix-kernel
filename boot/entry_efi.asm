section .text._start progbits alloc exec nowrite align=16

global _start_efi_amd64

extern _start
extern rust_main
extern setupPageTablesLongMode
extern l4_pml4
extern KERNEL_OFFSET

%define V2P(a) (a - KERNEL_OFFSET)  ; virtual to physical

MULTIBOOT2_BOOTLOADER_MAGIC equ 0x36D76289
MULTIBOOT_TAG_TYPE_EFI64    equ 12
EFI_SYSTEM_TABLE_CONOUT     equ 64
EFI_TEXT_OUTPUT_STRING      equ 8

_start_efi_amd64:
    cli
    call setupPageTablesLongMode
    ; after that new pml4 is set to only have kernel and eba regions mapped

    ; point cr3 to the new proper pml4
    mov rax, l4_pml4
    sub rax, KERNEL_OFFSET
    mov cr3, rax

    call rust_main
