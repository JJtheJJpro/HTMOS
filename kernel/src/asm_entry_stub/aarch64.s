.global _start
.extern htmkrnl
.extern __stack_end

.section .text._start, "ax"

_start:
    # Load the address of the stack end into the stack pointer (sp)
    ldr x0, =__stack_end
    mov sp, x0

    # Reset frame pointer (x29) and link register (x30)
    mov x29, #0
    mov x30, #0

    # Jump to the Rust kernel entry
    b htmkrnl