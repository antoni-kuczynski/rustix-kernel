BITS 32
global _start
; ====================================================
_start:
    mov esp, stack_top  ; set up the stack
    mov ah, 0   ; error code
    mov esi, ebx    ; store the multiboot struct address in esi

    call checkMultiboot
    call checkCPUID
    call checkLongMode
    call checkA20

    call setupPageTables
    call enable64BitPaging

    lgdt [GDT.Pointer]
    jmp 0x08:LongMode

    hlt

; ====================================================
enable64BitPaging:
    mov eax, page_table_l4    ; page table start
    mov cr3, eax


    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax    ; enable PAE

    mov ecx, 0xC0000080
    rdmsr
    or eax, 1 << 8
    wrmsr   ; long mode bit

    mov eax, cr0
    or eax, 1 << 31
    mov cr0, eax    ; enable paging

    ret
; ====================================================
setupPageTables:
    mov eax, page_table_l3
    or eax, 0b11    ; present, writeable
    mov [page_table_l4], eax

    mov eax, page_table_l2
    or eax, 0b11    ; present, writeable
    mov [page_table_l3], eax

    mov ecx, 0  ; counter for the loop

    .fillLoop:
        mov eax, 0x200000   ;2mb
        mul ecx
        or eax, 0b10000011  ; huge page flag
        mov [page_table_l2 + ecx * 8], eax

        inc ecx
        cmp ecx, 512
        jne .fillLoop   ; continue until the whole table is mapped
        ret


checkMultiboot:
    cmp eax, 0x36d76289
    jz .notMultibootError
    ret
    .notMultibootError:
        mov ah, "M"
        jmp error
; ====================================================
; checks for the presence of long mode
checkLongMode:
    mov eax, 0x80000000
    cpuid   ; get the highest extended CPUID leaf

    cmp eax, 0x80000001
    jb .noLongModeError    ; CPU didnt report lonf mode in its flags

    mov eax, 0x80000001
    cpuid
    test edx, (1 << 29) ; and the values to check if long mode is supported
    jz .noLongModeError ; if the bit is not set, the CPU doesnt support long mode
    ret

    .noLongModeError:
        mov ah, "L"
        jmp error
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
    jz .notSupported
    ret

    .notSupported:
        mov ah, "C"
        jmp error
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
error:
   cld
   mov edi, 0xB8000            ; VGA text buffer
   mov ecx, 1000               ; 4000 bytes / 4 = 1000 dwords
   mov eax, 0x1F201F20         ; ' ' with blue bg, white fg (twice)
   rep stosd                   ; Clear the screen


    mov byte [0xB8000], ah
    jmp .hlt
    .hlt:
        hlt
        jmp .hlt
; ====================================================
GDT:
    dq 0
    dq 0x00209A0000000000
    dq 0x0000920000000000

ALIGN 4
.Pointer:
    dw $ - GDT - 1
    dd GDT
; ====================================================
[BITS 64]
LongMode:
    mov ax, 0
    mov ss, ax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    mov ebx, esi    ; restore the multiboot struct address

    mov dword [0xB8000], ebx


    mov edi, 0xB8000
    mov ecx, 1000
    mov eax, 0x0F200F20
    rep stosd   ; clear the screen

    extern rust_main
    jmp rust_main
; ====================================================
section .bss
align 4096
page_table_l4:
    RESB 4096
page_table_l3:
    RESB 4096
page_table_l2:
    RESB 4096

stack_bottom:
    RESB 16384   ; 16kb stack space
stack_top:
