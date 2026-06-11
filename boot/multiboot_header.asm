BITS 64
section .multiboot2
align 8

extern _start
extern _start_efi_amd64

MB2_MAGIC    equ 0xE85250D6 ; magic number
MB2_ARCH     equ 0          ; 0 = 32‑bit protected mode
MB2_LENGTH   equ header_end - header_start ; header length
MB2_CHECKSUM equ -(MB2_MAGIC + MB2_ARCH + MB2_LENGTH)   ; cheksum

header_start:
    dd MB2_MAGIC ; magic
    dd MB2_ARCH ; architecture
    dd MB2_LENGTH ; header length
    dd MB2_CHECKSUM ; checksum

    ; console flags tag: console is optional, EGA text supported if available
    align 8
    dw 4    ; type 4
    dw 0    ; flags
    dd 12   ; size = 12
    dd 2    ; bit 1 = EGA text supported

    ; framebuffer tag: ask GRUB/UEFI for any available GOP framebuffer
    align 8
    dw 5    ; type 5
    dw 0    ; flags
    dd 20   ; size = 20
    dd 0    ; width: any
    dd 0    ; height: any
    dd 0    ; depth: any

    ; entry address tag
;    align 8
;    dw 3    ; type 3
;    dw 0    ; flags
;    dd 12   ; size = 12
;    dd _start

    ; efi amd64 entry address tag
    align 8
    dw 9    ; type 9
    dw 0    ; flags
    dd 12   ; size = 12
    dd _start_efi_amd64

    ; --- Required end tag ---
    align 8
    dw 0    ; type = 0 (end tag)
    dw 0    ; flags = 0
    dd 8    ; size = 8 bytes

header_end:
