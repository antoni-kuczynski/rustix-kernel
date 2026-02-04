BITS 32

section .text._start
global _start

; extern rust_main
; =======================================================================
PML4_TABLE_ADDR equ 0x1000  ; page map l4 table
PDPT_ADDR equ 0x2000    ; page directory pointer table
PDT_ADDR equ 0x3000 ; page directory table
PT_ADDR equ 0x4000  ; page table

PAGE_TABLE_SIZE equ 4096

PT_ADDR_MASK equ 0xffffffffff000
PT_PRESENT equ 1    ; marks the entry as in use
PT_READABLE equ 2   ; marks the entry as r/w

ENTRIES_PER_PT equ 512
SIZEOF_PT_ENTRY equ 8
PAGE_SIZE equ 0x1000

; Access bits
PRESENT        equ 1 << 7
NOT_SYS        equ 1 << 4
EXEC           equ 1 << 3
DC             equ 1 << 2
RW             equ 1 << 1
ACCESSED       equ 1 << 0

; Flags bits
GRAN_4K        equ 1 << 7
SZ_32          equ 1 << 6
LONG_MODE      equ 1 << 5
; =======================================================================
GDT:
gdt_null:                       ; selector 0x00
    dq 0

gdt_code:                       ; selector 0x08
    dw 0xFFFF                   ; limit low
    dw 0x0000                   ; base low
    db 0x00                     ; base mid
    db PRESENT | NOT_SYS | EXEC | RW
    db GRAN_4K | LONG_MODE | 0x0F  ; flags + high limit
    db 0x00                     ; base high

gdt_data:                       ; selector 0x10
    dw 0xFFFF                   ; limit low
    dw 0x0000                   ; base low
    db 0x00                     ; base mid
    db PRESENT | NOT_SYS | RW
    db GRAN_4K | SZ_32 | 0x0F   ; flags + high limit
    db 0x00                     ; base high

GDT_end:

GDT_Pointer:
    dw GDT_end - GDT - 1        ; limit
    dd GDT                      ; base (32‑bit)


; =======================================================================
_start:
    mov esp, stack_top  ; set the stack pointer to the top

    call checkCPUID  ; ax=1 if supported, 0 if not supported
    cmp ax, 0x01 ; check if CPUID is supported
    jne halt

    call checkForLongMode   ; check if CPU has long mode
    call checkA20   ; enable the A20 line
    call disable32BitPaging ; disable 32 bit protected mode paging
    call enable64BitPaging  ; enable the 64 bit paging
    call enablePAE_LM   ; enable PAE and long mode

    lgdt [GDT_Pointer]
    jmp 0x08:Realm64            ; 0x08 = gdt_code selector

    jmp halt
    ret
; =======================================================================
enablePAE_LM:
    ; enable PAE
    mov edx, cr4    ; control register 4
    or edx, (1 << 5)    ; enable PAE
    mov cr4, edx    ; restore

    ; set long mode enable, doesnt enable immediately, only permits it
    ; so that it can be enabled after paging is enabled
    mov ecx, 0xC0000080 ; msr index fo extended feature enable register
    rdmsr   ; load the msr into EAX and EDX
    or eax, (1 << 8)    ; set the Long Mode Enable bit
    wrmsr   ; write back the MSR

    ; re-enable paging
    ; mov eax, cr0    ; control register 0
    ; or eax, (1 << 31) ; enable 32 bit paging in CR0
    ; mov cr0, eax    ; restore cr0 with enabled paging

    ret

; =======================================================================
enable64BitPaging:
    ; we write 4 page tables = 2 megabytes in start address PML4_TABLE_ADDR
    mov edi, PML4_TABLE_ADDR    ; page table start
    mov cr3, edi

    xor eax, eax    ; eax = 0
    mov ecx, PAGE_TABLE_SIZE    ; set the counter for rep stosd
    rep stosd   ; writes 4 * PAGE_TABLE_SIZE = 4 page tables = 2 megabytes

    mov eax, cr3
    mov edi, eax
    ; mov edi, cr3    ;reset edi to the beggining of page table

    ; connect up the page entries
    ; edi is PML4T
    mov DWORD [edi], PDPT_ADDR & PT_ADDR_MASK | PT_PRESENT | PT_READABLE    ; connect PML4[0] -> PDPT

    mov edi, PDPT_ADDR
    mov DWORD [edi], PDT_ADDR & PT_ADDR_MASK | PT_PRESENT | PT_READABLE ; connect PDPT[0] -> PDT

    mov edi, PDT_ADDR
    mov DWORD [edi], PT_ADDR & PT_ADDR_MASK | PT_PRESENT | PT_READABLE  ; connect PDT[0] -> PT

    ; fill in the page table
    mov edi, PT_ADDR
    mov ebx, PT_PRESENT | PT_READABLE   ; initalize flags
    mov ecx, ENTRIES_PER_PT ; loop counter

    .setEntry:
        mov DWORD [edi], ebx    ; write address + flags
        add ebx, PAGE_SIZE  ; increase the address by page size
        add edi, SIZEOF_PT_ENTRY    ; move to the next entry
        loop .setEntry  ; until ecx is 0

; =======================================================================
disable32BitPaging:
    mov eax, cr0    ; control register 0
    and eax, ~(1 << 31) ; disable 32 bit paging in CR0
    mov cr0, eax    ; restore cr0 with disabled paging
    ret
; =======================================================================
; checks status of the A20 line and enables if
checkA20:
    pushad  ; push all 8 general purpose registers onto stack
    mov edi, 0x112345   ; odd megabyte address
    mov esi, 0x012345   ; even megabyte address

    ; move the values to both addresses and make sure they contain a different value
    mov [esi], esi
    mov [edi], edi
    cmpsd   ; compare esi and edi
    popad   ; restore registers
    je enableA20
    ret
; =======================================================================
; waits until the 8042 controller is ready to accept the command
a20commandWait:
    in al, 0x64
    test al,2
    jnz a20commandWait
    ret
; =======================================================================
; waits until the 8042 controller's is full
a20bufferWait:
    in al, 0x64
    test al, 1
    jz a20bufferWait
    ret
; =======================================================================
enableA20:
    cli

    ; disable the keyboard
    call a20commandWait
    mov al, 0xAD
    out 0x64, al

    ; read output port
    call a20commandWait
    mov al, 0xD0
    out 0x64, al

    call a20bufferWait
    in al, 0x60
    push eax

    ; write output port
    call a20commandWait
    mov al, 0xD1
    out 0x64, al

    ; modify the output port to enable A20
    call a20commandWait
    pop eax
    or al, 0x2
    out 0x60, al

    ; enable keyboard input
    call a20commandWait
    mov al, 0xAE
    out 0x64, al

    sti
    ret

; =======================================================================

; checks for the presence of long mode
checkForLongMode:
    mov eax, 0x80000000
    cpuid   ; get the highest extended CPUID leaf

    cmp eax, 0x80000001
    jb longModeNotSupported    ; CPU didnt report lonf mode in its flags

    mov eax, 0x80000001
    cpuid
    test edx, (1 << 29) ; and the values to check if long mode is supported
    jz longModeNotSupported ; if the bit is not set, the CPU doesnt support long mode

    ret
; =======================================================================

; checks for CPUID support by flipping the ID bit in EFLAGS register
checkCPUID:
    ;save the original EFLAGS register
    pushfd
    pop eax
    mov ecx, eax

    xor eax, 0x00200000 ; invert the 21 bit (ID bit) in EFLAGS
    push eax    ; save to EFLAGS
    popfd       ; restore the EFLAGS with flipped bit
    pushfd
    pop eax     ; save the modified EFLAGS

    ; restore original EFLAGS
    push ecx
    popfd

    ; check if the bit was flipped
    xor eax, ecx
    jnz .supported

    .notSupported:
        mov ax, 0
        ret
    .supported:
        mov ax, 1
        ret
; =======================================================================
longModeNotSupported:
    jmp halt
; =======================================================================
halt:   ; executes if the rust_main function returns
    hlt
    jmp halt
; =======================================================================

section .bss
align 16
stack_bottom:
    resb 4096
stack_top:


[BITS 64]
Realm64:
    ; load data segment
    mov ax, 0x10          ; data selector = second descriptor (GDT.Data)
    mov ds, ax
    mov es, ax
    mov ss, ax

    extern rust_main
    call rust_main

.hang:
    hlt
    jmp .hang
; =======================================================================
