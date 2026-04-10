.global _start
.extern htmkrnl
.extern __stack_end

.section .text._start, "ax"

_start:
    mov esp, __stack_end
    xor ebp, ebp
    jmp htmkrnl