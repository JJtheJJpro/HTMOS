ENTRY(entry)
OUTPUT_FORMAT(binary)

SECTIONS
{
    /* Stage 2 is loaded here by boot.asm */
    . = 0x8000;

    .text :
    {
        *(.entry)         /* Your assembly entry point */
        *(.text .text.*)  /* All Rust and other ASM functions */
    }

    /* Merge read-only and standard data */
    .rodata : { *(.rodata .rodata.*) }
    .data   : { *(.data .data.*) }

    /* The GOT must be included in the binary output */
    .got    : { *(.got) }

    /* BSS doesn't take space in the file, but takes space in RAM */
    .bss    : 
    { 
        __bss_start = .;
        *(.bss .bss.*) 
        *(COMMON)
        __bss_end = .;
    }

    /* Keep track of the end of the binary for stack safety */
    _end = .;
}