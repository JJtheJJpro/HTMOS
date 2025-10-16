.global _start
.extern htmkrnl
.extern __stack_end

.section .text._start, "ax"

_start:
    lea rsp, [rip + __stack_end]
    xor rbp, rbp
    jmp htmkrnl