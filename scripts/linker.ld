KERNEL_OFFSET = 0xFFFFFFFF80000000;
KERNEL_PHYS_BASE = 0x00100000;

SECTIONS {
    . = KERNEL_PHYS_BASE;

    .boot ALIGN(4K) : AT(ADDR(.boot))
    {
        KEEP(*(.multiboot2_header))
        KEEP(*(.multiboot2))
        KEEP(*(.text._start))
        KEEP(*(.boot .boot.*))
    }

    . = ALIGN(4K);
    boot_end = .;

    . += KERNEL_OFFSET;

    . = ALIGN(4K);
    .text : AT(ADDR(.text) - KERNEL_OFFSET)
    {
        *(.text .text.*)
    }

    . = ALIGN(4K);
    .rodata : AT(ADDR(.rodata) - KERNEL_OFFSET)
    {
        *(.rodata .rodata.*)
    }

    . = ALIGN(4K);
    .data : AT(ADDR(.data) - KERNEL_OFFSET)
    {
        *(.data .data.*)
    }

    . = ALIGN(4K);
    .bss : AT(ADDR(.bss) - KERNEL_OFFSET)
    {
        *(COMMON)
        *(.bss .bss.*)
    }

    . = ALIGN(4K);
    endKernel = . - KERNEL_OFFSET;
}
