#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct DirectoryEntry {
    name: [u8; 11],
    attr: u8,
    _reserved: [u8; 8],
    cluster_high: u16,
    _time: u16,
    _date: u16,
    cluster_low: u16,
    size: u32,
}

unsafe extern "C" {
    unsafe fn bios_read_sector(drive: u16, lba_low: u32, lba_high: u32, buffer: *mut u8) -> u16;
}

#[unsafe(no_mangle)]
pub fn locate_kernel(drive_id: u8) -> u32 {
    let mut buffer = [0u8; 512];

    // 1. Read the BPB (Sector 0)
    unsafe {
        bios_read_sector(drive_id as u16, 0, 0, buffer.as_mut_ptr());
    }

    // FAT32 offsets from the BPB
    let bytes_per_sector = u16::from_le_bytes([buffer[11], buffer[12]]) as u32;
    let sectors_per_cluster = buffer[13] as u32;
    let reserved_sectors = u16::from_le_bytes([buffer[14], buffer[15]]) as u32;
    let num_fats = buffer[16] as u32;
    let sectors_per_fat = u32::from_le_bytes([buffer[36], buffer[37], buffer[38], buffer[39]]);
    let root_cluster = u32::from_le_bytes([buffer[44], buffer[45], buffer[46], buffer[47]]);

    // 2. Calculate where the Data Area starts
    // First Data Sector = Reserved + (NumFATs * SectorsPerFAT)
    let fat_begin_lba = reserved_sectors;
    let cluster_begin_lba = reserved_sectors + (num_fats * sectors_per_fat);

    // 3. Find the Root Directory Sector
    // In FAT32, the root dir is just a cluster chain starting at root_cluster
    let root_dir_lba = cluster_begin_lba + (root_cluster - 2) * sectors_per_cluster;

    // 4. Search for "KERNEL  " (8.3 format: 8 chars name, 3 chars ext)
    unsafe {
        bios_read_sector(drive_id as u16, root_dir_lba, 0, buffer.as_mut_ptr());
    }

    let mut kernel_cluster = 0u32;
    let entries = unsafe { core::mem::transmute::<&[u8; 512], &[DirectoryEntry; 16]>(&buffer) };

    for entry in entries.iter() {
        if entry.name[..8] == *b"kernel  " {
            kernel_cluster = ((entry.cluster_high as u32) << 16) | (entry.cluster_low as u32);
            break;
        }
    }

    if kernel_cluster == 0 {
        return 0; // Kernel not found
    }

    // 5. Load the Kernel into a buffer at 0x20000
    // For simplicity, this loads the first cluster of the file.
    // If your kernel is > 4KB, you'd loop through the FAT table here.
    let kernel_lba = cluster_begin_lba + (kernel_cluster - 2) * sectors_per_cluster;
    let dest_buffer = 0x20000 as *mut u8;

    for i in 0..sectors_per_cluster {
        unsafe {
            bios_read_sector(
                drive_id as u16,
                kernel_lba + i,
                0,
                dest_buffer.add(i as usize * 512),
            );
        }
    }

    0x20000 // Return address of loaded ELF
}

#[unsafe(no_mangle)]
pub extern "C" fn kmain(elf_address: u32) -> ! {
    let vga_buffer = 0xb8000 as *mut u16;

    unsafe {
        let text = b"HTMOS BIOS BOOTLOADER";
        let color = 0x0E; // Yellow on Black (easy to see)

        for (i, &byte) in text.iter().enumerate() {
            // Combine color (high byte) and char (low byte)
            let value = (color << 8) | (byte as u16);
            vga_buffer.add(i + 40).write_volatile(value); // Offset slightly to see ABCD
        }
    }

    if elf_address != 0 {
        unsafe {
            let text = b"GOOD";
            let color = 0x0E; // Yellow on Black (easy to see)

            for (i, &byte) in text.iter().enumerate() {
                let value = (color << 8) | (byte as u16);
                vga_buffer.add(i).write_volatile(value);
            }
        }
    }

    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
