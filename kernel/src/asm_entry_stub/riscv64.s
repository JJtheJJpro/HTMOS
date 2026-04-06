.global _start
.extern htmkrnl
.extern __stack_end

.section .text._start, "ax"

_start:
    # Set the stack pointer
    la sp, __stack_end

    # Clear the frame pointer (s0/fp)
    li s0, 0

    # Jump to Rust
    tail htmkrnl