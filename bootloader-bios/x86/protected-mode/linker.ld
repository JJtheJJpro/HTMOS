ENTRY(_start)

SECTIONS
{
    . = 0x7E00 + 0x4800;

    /* The code section */
    .text : AT(0x7E00 + 0x4800)
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
        *(.eh_frame_hdr)
        *(.rel.eh_frame)
        *(.rel.eh_frame_hdr)
    }
}