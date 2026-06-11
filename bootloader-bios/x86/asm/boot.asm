[BITS 16]
[ORG 0x7C00]

start:
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7c00

    mov ah, 0x41
    mov bx, 0x55AA
    int 0x13
    jc disk_error
    cmp bx, 0xAA55
    jnz disk_error

    mov ah, 0x42
    mov si, dap
    int 0x13
    jc disk_error

    jmp 0x0000:0x7e00

disk_error:
    mov ah, 0x0e
    mov al, 'E'
    int 0x10
    hlt

align 4
dap:
    db 0x10
    db 0
    dw 55 ; count
    dw 0x7E00
    dw 0x0000
    dq 34 ; start

times 510-($-$$) db 0
dw 0xAA55