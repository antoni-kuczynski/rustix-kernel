BITS 64
section .multiboot2
align 8

MB2_MAGIC    equ 0xE85250D6 ; magic number
MB2_ARCH     equ 0          ; 0 = 32‑bit protected mode
MB2_LENGTH   equ header_end - header_start ; header length
MB2_CHECKSUM equ -(MB2_MAGIC + MB2_ARCH + MB2_LENGTH)   ; cheksum

header_start:
    dd MB2_MAGIC ; magic
    dd MB2_ARCH ; architecture
    dd MB2_LENGTH ; header length
    dd MB2_CHECKSUM ; checksum

    ; --- Required end tag ---
    dw 0    ; type = 0 (end tag)
    dw 0    ; flags = 0
    dd 8    ; size = 8 bytes

header_end:
