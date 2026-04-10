[org 0x7c00]

; --- FAT32 Header Start ---
; The BIOS jumps here, and we immediately jump over the BPB data
jmp short start
nop

; This is the BPB (BIOS Parameter Block). 
; mkfs.fat will overwrite these values with the correct ones for your USB.
; We use 'db 0' or 'dw 0' as placeholders.
oem_name            db "MSWIN4.1"   ; 8 bytes
bytes_per_sector    dw 0
sectors_per_cluster db 0
reserved_sectors    dw 0
fat_count           db 0
root_entries        dw 0
total_sectors_16    dw 0
media_type          db 0
sectors_per_fat_16  dw 0
sectors_per_track   dw 0
heads_count         dw 0
hidden_sectors      dd 0
total_sectors_32    dd 0

; FAT32 Extended Boot Record
sectors_per_fat_32  dd 0
ext_flags           dw 0
fs_version          dw 0
root_cluster        dd 0
fs_info_sector      dw 0
backup_boot_sector  dw 0
reserved            times 12 db 0
drive_number        db 0
reserved1           db 0
boot_signature      db 0x29
volume_id           dd 0
volume_label        db "HTMOS BOOT " ; 11 bytes
file_system_type    db "FAT32   "    ; 8 bytes
; --- FAT32 Header End ---

start:
    ; 1. Fix Segments & Stack
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7c00
    sti

    ; 2. Save the boot drive ID passed by BIOS in DL
    mov [BOOT_DRIVE], dl

    ; 3. Print 'A' (MBR started)
    mov ax, 0x0e41
    int 0x10

    ; TESTING (making sure 'A' still prints)
    jmp $

    ; 4. Read Stage 2 from disk
    ; We are reading from the "Reserved Sectors" area.
    ; This is safe space before the actual FAT tables start.
    mov ah, 0x02    
    mov al, 30      ; Read 30 sectors (approx 15KB)
    mov ch, 0x00    
    mov dh, 0x00    
    mov cl, 0x02    ; Sector 2 (Immediately after MBR)
    mov dl, [BOOT_DRIVE]
    mov bx, 0x8000  
    int 0x13
    jc disk_error   

    ; 5. Jump to Stage 2
    mov dl, [BOOT_DRIVE]
    jmp 0x8000

disk_error:
    mov ax, 0x0e45  
    int 0x10
    jmp $

; Fill the rest of the sector, leaving 3 bytes for BOOT_DRIVE and Signature
times 510-($-$$)-1 db 0 
BOOT_DRIVE db 0
dw 0xaa55