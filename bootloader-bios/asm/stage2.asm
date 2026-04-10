section .entry
[bits 16]
global entry
extern locate_kernel
extern kmain

entry:
    ; 1. B, C, D are already confirmed
    mov ax, 0x0e42
    int 0x10
    
    ; A20 (Fast Gate)
    in al, 0x92
    or al, 2
    out 0x92, al
    mov ax, 0x0e43
    int 0x10

    ; DL was set in boot.asm right before the jmp 0x8000
    movzx ax, dl          ; Move drive ID into AX
    push ax               ; Push it as the argument for locate_kernel(drive_id)
    call locate_kernel    ; Call Rust while still in 16-bit mode!
    add sp, 2             ; Clean up the stack

    ; EAX now contains 0x20000 (the address where Rust loaded the ELF)
    ; We need to save this EAX so we can use it after the PM switch
    mov esi, eax

    ; Load GDT
    cli
    lgdt [gdt_descriptor]
    mov ax, 0x0e44
    int 0x10

    ; 2. Switch to Protected Mode
    mov eax, cr0
    or eax, 1
    mov cr0, eax

    ; 3. Far Jump to flush the pipeline and load CS
    ; 0x08 is the offset of the Code Descriptor in our GDT
    jmp 0x08:init_pm

[bits 32]
init_pm:
    ; 4. Setup segment registers with the Data Selector (0x10)
    mov ax, 0x10
    mov ds, ax
    mov ss, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    ; 5. Set up a 32-bit stack (Crucial for physical hardware)
    mov esp, 0x90000

    ; 6. Write 'E' directly to VGA Buffer
    ; Address 0xb8000 is the start of text mode memory.
    ; Each character takes 2 bytes: [ASCII][Attribute]
    ; We'll skip the first few spots so we don't overwrite ABCD
    mov edx, 0xb8000
    mov [edx + 8], byte 'E'     ; The character
    mov [edx + 9], byte 0x0F    ; Attribute: White on Black

    ; 2. Call the Rust entry point
    call kmain

    ; Hang forever
    jmp $

; --- GDT remains the same ---
gdt_start:
    dq 0x0
gdt_code:
    dw 0xffff, 0x0000, 0x9a00, 0x00cf 
gdt_data:
    dw 0xffff, 0x0000, 0x9200, 0x00cf 
gdt_end:

gdt_descriptor:
    dw gdt_end - gdt_start - 1
    dd gdt_start

; --- Fix your bios_read_sector bridge ---
; It needs to use 16-bit style stack access because it's called in 16-bit mode
global bios_read_sector
bios_read_sector:
    push bp
    mov bp, sp
    push si

    ; DAP on stack (16-bit offsets)
    push dword [bp + 8]   ; LBA High
    push dword [bp + 4]   ; LBA Low
    push word 0           ; Segment
    push word [bp + 12]   ; Buffer Offset
    push word 1           ; Count
    push word 0x0010      ; Size

    mov si, sp
    mov dl, [bp + 2]      ; Drive ID passed from Rust
    mov ah, 0x42
    int 0x13
    
    jc .error
    xor ax, ax
    jmp .done
.error:
    movzx ax, ah
.done:
    add sp, 16
    pop si
    pop bp
    ret