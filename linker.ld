ENTRY(_start)

SECTIONS {
    /* Physical load address */
    . = 1M;

    /* First loadable segment */
    .boot :
    {
        KEEP(*(.multiboot2_header))
        KEEP(*(.multiboot2))
        *(.text._start)   /* ensure _start is here */
        *(.text*)
    }

    .rodata :
    {
        *(.rodata*)
    }

    .data :
    {
        *(.data*)
    }

    .bss :
    {
        *(.bss*)
        *(COMMON)
        endKernel = .;
    }
}
