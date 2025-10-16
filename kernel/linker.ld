ENTRY(_start)

STACK_SIZE = 32K;

PHDRS {
    text PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
}

SECTIONS
{
    /* Load at 1 MiB */
    . = 1M;

    __kernel_start = .;
    .text : ALIGN(4K) {
        *(.text._start)   /* ensure _start is here */
        *(.text .text.*)
    } :text

    .rodata : ALIGN(4K) {
        *(.rodata .rodata.*)
    } :text

    .data : ALIGN(4K) {
        *(.data .data.*)
    } :data

    .bss : ALIGN(4K) {
        *(.bss .bss.*)
        *(COMMON)
    } :data
    __kernel_end = .;

    .stack (NOLOAD) : ALIGN(4K)
    {
        __stack_start = .;
        . += STACK_SIZE;
        __stack_end = .;
    } :data
}
