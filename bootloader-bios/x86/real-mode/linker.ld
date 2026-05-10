ENTRY(_start)

SECTIONS
{
    /* We told the BIOS to load us at 0x7E00 */
    . = 0x7E00;

    /* The code section */
    .text :
    {
        /* Ensure the entry point is at the very beginning of the binary */
        *(.text._start)
        *(.text .text.*)
    }

    /* Read-only data (like strings) */
    .rodata :
    {
        *(.rodata .rodata.*)
    }

    /* Initialized data */
    .data :
    {
        *(.data .data.*)
    }

    /* Uninitialized data (zeroed) */
    .bss :
    {
        *(.bss .bss.*)
    }

    /* Discard sections we don't need for a raw 16-bit binary */
    /DISCARD/ :
    {
        *(.comment)
        *(.eh_frame)
        *(.rel.eh_frame)
    }
}