ENTRY(_start)

KERNEL_OFFSET = 0xFFFFFFFF80000000;

SECTIONS {
    /* Physical load address */
    . = 0x00100000;


    /* First loadable segment */
    .boot :
    {
        KEEP(*(.multiboot2_header))
        KEEP(*(.multiboot2))
        *(.text._start)
    }

    . += KERNEL_OFFSET;

    .text ALIGN (4K) : AT (ADDR (.text) - KERNEL_OFFSET)
    {
        *(.text)
    }

    .rodata ALIGN (4K) : AT (ADDR (.rodata) - KERNEL_OFFSET)
    {
        *(.rodata*)
    }

    .data ALIGN (4K) : AT (ADDR (.data) - KERNEL_OFFSET)
    {
        *(.data*)
    }

    .bss ALIGN (4K) : AT (ADDR (.bss) - KERNEL_OFFSET)
    {
        *(.bss*)
        *(COMMON)
        endKernel = . - KERNEL_OFFSET;
    }
}
