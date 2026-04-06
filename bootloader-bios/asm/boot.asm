[org 0x7c00]
jmp short start
nop

; --- BIOS Parameter Block (BPB) ---
OEMIdentifier           db "MSWIN4.1"
BytesPerSector          dw 512
SectorsPerCluster       db 1
ReservedSectors         dw 1
TotalFATs               db 2
MaxRootEntries          dw 224
TotalSectorsSmall       dw 2880
MediaDescriptor         db 0xF0
SectorsPerFAT           dw 9
SectorsPerTrack         dw 18
NumberofHeads           dw 2
HiddenSectors           dd 0
TotalSectorsLarge       dd 0
DriveNumber             db 0x80
Signature               db 0x29
VolumeID                dd 0x12345678
VolumeLabel             db "BOOT OS    "
SystemID                db "FAT12   "

start:
    mov [BOOT_DRIVE], dl    ; Save boot drive

    ; Setup segments
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7c00
    sti

    ; Breadcrumb 'A': MBR Started
    mov ax, 0x0e41          ; 'A'
    int 0x10

    call load_stage2

    ; Breadcrumb 'B': Stage 2 Loaded
    mov ax, 0x0e42          ; 'B'
    int 0x10

    jmp 0x0000:0x8000       ; Jump to Stage 2

load_stage2:
    mov ah, 0x02
    mov al, 1               ; Read 1 sector
    mov ch, 0x00
    mov dh, 0x00
    mov cl, 0x02            ; Sector 2
    mov dl, [BOOT_DRIVE]
    mov bx, 0x8000          ; Load to 0x8000
    int 0x13
    jc disk_error
    ret

disk_error:
    mov ax, 0x0e45          ; 'E' for Error
    int 0x10
    jmp $

BOOT_DRIVE db 0
times 510-($-$$) db 0
dw 0xaa55