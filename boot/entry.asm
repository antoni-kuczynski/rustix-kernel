BITS 32
global _start

; ====================================================
KERNEL_OFFSET equ 0xFFFFFFFF80000000    ; kernel memory offset
PHYS_BASE equ 0x00100000

extern endKernel
%define V2P(a) (a - KERNEL_OFFSET)  ; virtual to physical
%define P2V(a) (a + KERNEL_OFFSET)  ; physical to virtual
; ====================================================
_start:
    mov esp, V2P(stack_top)  ; set up the stack
    mov ah, 0   ; error code
    mov esi, ebx    ; store the multiboot struct address in esi

    call checkMultiboot
    call checkCPUID
    call checkLongMode
    call checkA20

    call setupPageTables
    call enable64BitPaging

    lgdt [V2P(GDT.Pointer)]
    jmp 0x08:V2P(LongMode)

    hlt
; ====================================================
enable64BitPaging:
    mov eax, V2P(l4_pml4)    ; page table start
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
    mov eax, V2P(l3_pdpt_low)
    or eax, 0b11
    mov [V2P(l4_pml4)], eax

    mov eax, V2P(l2_pd_low)
    or eax, 0b11
    mov [V2P(l3_pdpt_low)], eax

    mov ecx, 0

    .fillLoop:
        mov eax, 0x200000   ;2mb
        mul ecx
        or eax, 0b10000011  ; huge page flag
        mov [V2P(l2_pd_low) + ecx * 8], eax

        inc ecx
        cmp ecx, 512
        jne .fillLoop   ; continue until the whole table is mapped
        ret

; ====================================================
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
    mov ebx, 0x012345   ; even megabyte address

    ; move the values to both addresses and make sure they contain a different value
    mov [ebx], ebx
    mov [edi], edi
    cmpsd   ; compare ebx and edi
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
    dw V2P($ - GDT - 1)
    dd V2P(GDT)
; ====================================================
[BITS 64]
LongMode:
    mov ax, 0
    mov ss, ax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    call setupPageTablesLongMode

    movabs rax, higherHalfMemory
    jmp rax

    hlt
; ====================================================
higherHalfMemory:
    ; add kernel offset to stack pointer
    mov rax, KERNEL_OFFSET
    add rsp, rax

    ; remove the idendity mapping
    mov rax, 0
    mov qword [V2P(l4_pml4)], rax

    ; flush page table
    mov rax, cr3
    mov cr3, rax

    mov [__oldMultibootPhysAddr], esi   ; restore the multiboot struct address

    call vgaInit

    extern rust_main
    call rust_main

    hlt
; ====================================================
setupPageTablesLongMode:
    ;--------------------------------------------
    ; higher half kernel page tables
    ; --------------------------------------------
    ; map kernel l3 pdpt
    mov rax, V2P(l3_pdpt_kernel)
    or rax, 0b11
    mov qword [V2P(l4_pml4) + 511*8], rax ; 9 bits equal to 1 = 511
    ; ===========================

    ; map kernel l2 pd
    mov rax, V2P(l2_pd_kernel)
    or rax, 0b11
    mov [V2P(l3_pdpt_kernel) + 510*8], rax ; 8 msb bits 1, lsb=0, = 510
    ; ===========================

    ; --------------------------------------------
    .kernelPageTables:
        xor rcx, rcx
        mov rdi, 0x00000000

        lea rbx, [endKernel]
        ; ===========================
        .kernelMapLoop:
            mov rax, rdi
            or rax, 0b11    ; present, writeable flags
            or rax, (1 << 7)    ; huge page flag
            mov qword [V2P(l2_pd_kernel) + rcx*8], rax


            add rdi, 0x200000   ; add 2mb
            inc rcx
            cmp rdi, rbx
            jb .kernelMapLoop
        ; ===========================
    ; --------------------------------------------

    ; --------------------------------------------
    tempHeapPageTables:
        lea rdi, [earlyHeapStart]   ; set early heap start
        lea rax, [endKernel]    ; end kernel address
        add rax, 0x200000   ; add size of page
        and rax, ~(0x200000 - 1) ; align to page
        mov [rdi], rax          ; put in .bss

        lea rdi, [earlyHeapEnd] ; end of temp heap
        add rax, 0x800000       ; size of temp heap = 8mb
        mov [rdi], rax          ; put in .bss


        ; counter is rcx, which is left at value of last allocated page for the kernel
        ; from previous loop
        ; here we map a temp heap memory region of 8mb, right after kernel end
        mov rdi, [earlyHeapStart] ; start address - already page aligned
        mov rbx, [earlyHeapEnd] ; end address - already page aligned
        ; ===========================
        .earlyHeapMapLoop:
            mov rax, rdi
            or rax, 0b11    ; present, writeable flags
            or rax, (1 << 7)    ; huge page flag
            mov qword [V2P(l2_pd_kernel) + rcx*8], rax

            add rdi, 0x200000   ; still using 2mb pages
            inc rcx
            cmp rdi, rbx
            jb .earlyHeapMapLoop
        ; ===========================
    ; --------------------------------------------
    ret
; ====================================================
vgaInit:
    ; clear the screen
    mov rdi, P2V(0xB8000)
    mov qword [rdi], 0x0
    mov rcx, 1000
    mov rax, 0x0F200F20
    rep stosd

    ; set the cursor to first char
    mov dx, 0x3D4
    mov al, 0x0F        ; cursor low byte index
    out dx, al

    mov dx, 0x3D5
    mov al, 0           ; low byte of position
    out dx, al

    mov dx, 0x3D4
    mov al, 0x0E        ; cursor high byte index
    out dx, al

    mov dx, 0x3D5
    mov al, 0           ; high byte of position
    out dx, al

    ret
; ====================================================
section .bss
; ====================================================
align 4096
l4_pml4:
    RESB 4096
; ====================================================
align 4096
l3_pdpt_low:
    RESB 4096
; ====================================================
align 4096
l3_pdpt_kernel:
    RESB 4096
; ====================================================
align 4096
l2_pd_low:
    RESB 4096
; ====================================================
align 4096
l2_pd_kernel:
    RESB 4096
; ====================================================
align 4096
l1_pt_low:
    RESB 4096
; ====================================================
global earlyHeapStart
global earlyHeapEnd
global __oldMultibootPhysAddr
; ====================================================
earlyHeapStart:
    RESQ 1
; ====================================================
earlyHeapEnd:
    RESQ 1
; ====================================================
__oldMultibootPhysAddr:
    RESD 1  ; 4 bytes
; ====================================================

stack_bottom:
    RESB 16384   ; 16kb stack space
stack_top:
