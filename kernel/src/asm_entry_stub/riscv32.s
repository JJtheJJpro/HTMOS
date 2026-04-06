.global _start
.extern htmkrnl
.extern __stack_end

.section .text._start, "ax"

_start:
    # Set the stack pointer (32-bit)
    la sp, __stack_end

    # Clear the frame pointer (s0/fp)
    li s0, 0

    # Tail call to Rust entry
    tail htmkrnl