ENTRY(entry)
OUTPUT_FORMAT(binary)

SECTIONS
{
    . = 0x8000;

    .text :
    {
        /* Force the entry assembly to the front */
        *(.entry)
        *(.text .text.*)
    }

    .rodata : { *(.rodata .rodata.*) }
    .data   : { *(.data .data.*) }
    .bss    : { *(.bss .bss.*) }
}