#![no_main]
#![no_std]

extern crate alloc;

mod console;
mod guids;
mod ui;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
//use custon_guid::AMI_ROM_LAYOUT_GUID;
use uefi::{
    boot::{MemoryType, OpenProtocolAttributes, OpenProtocolParams}, fs::FileSystem, prelude::*, print, println, proto::console::gop::GraphicsOutput, system, table::cfg::{
        ACPI2_GUID, ACPI_GUID, DEBUG_IMAGE_INFO_GUID, DXE_SERVICES_GUID, ESRT_GUID, HAND_OFF_BLOCK_LIST_GUID, LZMA_COMPRESS_GUID, MEMORY_STATUS_CODE_RECORD_GUID, MEMORY_TYPE_INFORMATION_GUID, PROPERTIES_TABLE_GUID, SMBIOS3_GUID, SMBIOS_GUID, TIANO_COMPRESS_GUID
    }, CStr16, CString16
};

// helper function:
fn cstr16_from_fmt(args: core::fmt::Arguments<'_>) -> uefi::Result<CString16> {
    // first get a UTF-8 String
    let mut s = String::new();
    core::fmt::write(&mut s, args)
        .map_err(|_| uefi::Status::ABORTED)
        .unwrap();
    // then convert to UCS-2 + null terminator
    let asdf = CString16::try_from(s.as_str()).unwrap();
    Ok(asdf)
}

// a little macro to make it ergonomic:
macro_rules! cformat16 {
    ($($tt:tt)*) => {
        cstr16_from_fmt(format_args!($($tt)*))
            .expect("failed to convert to CStr16")
    };
}

fn walk_fs(fs: &mut FileSystem, prefix: &CStr16) -> uefi::Result {
    let mut iter = fs.read_dir(prefix).unwrap();
    while let Some(entry) = iter.next() {
        let entryt = entry.unwrap();

        let name = entryt.file_name();
        if name == cstr16!(".") || name == cstr16!("..") {
            continue;
        }
        let path = if prefix == cstr16!("\\") {
            cformat16!("\\{name}")
        } else {
            cformat16!("{prefix}\\{name}")
        };
        println!("{}", path);
        if !entryt.is_regular_file() {
            walk_fs(fs, &path).unwrap();
        }
    }

    Ok(())
}

#[entry]
fn main() -> Status {
    if let Err(e) = uefi::helpers::init() {
        println!("{:?}", e.data());
        return e.status();
    }

    if let Err(e) = console::clear() {
        return e.status();
    }

    /*
    console::readline().unwrap();
    {
        let partition_buf =
            boot::locate_handle_buffer(SearchType::ByProtocol(&PartitionInfo::GUID)).unwrap();
        for (_i, &handle) in partition_buf.iter().enumerate() {
            let proto = boot::open_protocol_exclusive::<PartitionInfo>(handle).unwrap();
            if let Some(entry) = proto.gpt_partition_entry() {
                let asdf = entry.partition_name;
                let raw: &CStr16 = CStr16::from_char16_until_nul(&asdf).unwrap();
                println!("gptname: \"{raw}\"");
            }
        }
    }
    console::readline().unwrap();
    {
        let fs_buf =
            boot::locate_handle_buffer(SearchType::ByProtocol(&SimpleFileSystem::GUID)).unwrap();
        for (_i, &handle) in fs_buf.iter().enumerate() {
            let mut proto: boot::ScopedProtocol<SimpleFileSystem> =
                boot::open_protocol_exclusive(handle).unwrap();
            let mut root = proto.open_volume().unwrap();
            let mut fs = FileSystem::new(proto);
            walk_fs(&mut fs, cstr16!("\\")).unwrap();

            //let dp = boot::open_protocol_exclusive::<DevicePath>(handle).unwrap();
            //println!("{}", dp.to_string(DisplayOnly(true), AllowShortcuts(true)).unwrap());

            let mut buf = [0u8; 512];
            let fs_info = root.get_info::<FileSystemInfo>(&mut buf).unwrap();
            println!("label: \"{}\"", fs_info.volume_label().to_string());

            console::readline().unwrap();
        }
    }
    */

    let rev_firm = system::firmware_revision();
    println!(
        "{} {}.{}",
        system::firmware_vendor(),
        rev_firm >> 16,
        rev_firm & 0xFFFF
    );

    let rev_uefi = system::uefi_revision();
    println!("UEFI {}.{}", rev_uefi.major(), rev_uefi.minor());

    println!("UEFI Configuration Tables:");
    system::with_config_table(|config_table| {
        for entry in config_table {
            let guid = entry.guid;
            let addr = entry.address as usize;
            let label = if guid == ACPI2_GUID {
                "ACPI 2.0"
            } else if guid == SMBIOS3_GUID {
                "SMBIOS 3.x"
            } else if guid == ACPI_GUID {
                "ACPI"
            } else if guid == ESRT_GUID {
                "ESRT"
            } else if guid == SMBIOS_GUID {
                "SMBIOS"
            } else if guid == PROPERTIES_TABLE_GUID {
                "PROPERTIES"
            } else if guid == DXE_SERVICES_GUID {
                "DXE SERVICES"
            } else if guid == DEBUG_IMAGE_INFO_GUID {
                "DEBUG IMAGE INFO"
            } else if guid == HAND_OFF_BLOCK_LIST_GUID {
                "HAND OFF BLOCK LIST"
            } else if guid == MEMORY_TYPE_INFORMATION_GUID {
                "MEMORY TYPE INFO"
            } else if guid == MEMORY_STATUS_CODE_RECORD_GUID {
                "MEMORY STATUS CODE RECORD"
            } else if guid == LZMA_COMPRESS_GUID {
                "LZMA"
            } else if guid == TIANO_COMPRESS_GUID {
                "TIANO"
            //} else if guid == AMI_ROM_LAYOUT_GUID {
            //    "AMI ROM LAYOUT"
            } else {
                &guid.to_string()
            };
            println!("  {:<36} @ {:#010X}", label, addr);
        }
    });

    //let mmap = boot::memory_map(MemoryType::LOADER_DATA).unwrap();
    //println!("Memory Map ({} entries):", mmap.len());
    //for map in mmap.entries() {
    //    println!(
    //        "  Type: {:?}, PhysStart: {:#010X}, Pages: {}, Attr: {:#X}",
    //        map.ty, map.phys_start, map.page_count, map.att
    //    )
    //}

    system::with_config_table(|config_table| {
        for entry in config_table {
            if entry.guid == SMBIOS3_GUID {
                let table_ptr = entry.address as *const u8;
                println!("SMBIOS3 Table @ {:#010X}", table_ptr as usize);
                println!();

                let max_size = unsafe { *(table_ptr.add(0x0C) as *const u32) as usize };

                let struct_table_ptr = unsafe {
                    let addr_ = *(table_ptr.add(0x10) as *const u64);
                    addr_ as *const u8
                };

                let table_slice =
                    unsafe { core::slice::from_raw_parts(struct_table_ptr, max_size) };
                let mut data = Vec::with_capacity(max_size);
                data.extend_from_slice(table_slice);
                println!("{:?}", data);
            }
        }
    });

    let (w, h) = ui::get_resolution();
    println!("Screen Resolution: {w}x{h}");

    print!("Press enter to exit...");
    let input = match console::readline() {
        Ok(r) => r,
        Err(e) => return e.status(),
    };
    println!("{input}");

    let gop_handle = boot::get_handle_for_protocol::<GraphicsOutput>().unwrap();
    let mut gop = unsafe {
        boot::open_protocol::<GraphicsOutput>(
            OpenProtocolParams {
                handle: gop_handle,
                agent: boot::image_handle(),
                controller: None,
            },
            OpenProtocolAttributes::GetProtocol,
        )
        .unwrap()
    };

    let mut screen = ui::Buffer::current();
    screen.rect(&mut gop, 0, 0, w, h, (0, 255, 0)).unwrap();
    boot::stall(5_000_000);
    screen.rect(&mut gop, 0, 0, w, h, (0, 0, 0)).unwrap();

    console::clear().unwrap();
    println!("test1");
    drop(screen);
    println!("test2");
    drop(gop);
    println!("test3");

    //unsafe { boot::exit_boot_services(MemoryType::LOADER_DATA); }
    Status::SUCCESS
}
