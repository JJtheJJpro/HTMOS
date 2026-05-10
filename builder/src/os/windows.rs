use super::DeviceInfo;
use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_FILE_NOT_FOUND, GENERIC_READ, HANDLE},
        Storage::FileSystem::{
            BusTypeUsb, CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        },
        System::{
            IO::DeviceIoControl,
            Ioctl::{
                DISK_GEOMETRY_EX, IOCTL_DISK_GET_DRIVE_GEOMETRY_EX, IOCTL_STORAGE_QUERY_PROPERTY,
                PropertyStandardQuery, STORAGE_DEVICE_DESCRIPTOR, STORAGE_PROPERTY_QUERY,
                StorageDeviceProperty,
            },
        },
    },
    core::PCWSTR,
};

fn device_indexes() -> Vec<u32> {
    let mut found = Vec::new();

    for i in 0..128 {
        let path = format!("\\\\.\\PhysicalDrive{}", i);
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

        let result = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                Default::default(),
                None,
            )
        };

        match result {
            Ok(handle) => {
                unsafe { CloseHandle(handle).unwrap() };
                found.push(i);
            }
            Err(e) => {
                // ERROR_FILE_NOT_FOUND means no more drives exist beyond this point
                // ERROR_ACCESS_DENIED means it exists but we can't open it (still count it)
                if e.code() == ERROR_FILE_NOT_FOUND.into() {
                    break; // safe to stop
                }
                // Any other error (access denied, busy) — drive exists, just note it
                found.push(i);
            }
        }
    }

    found
}

fn get_drive_info(handle: HANDLE) -> Option<String> {
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        AdditionalParameters: [0],
    };

    let mut buf = vec![0u8; 1024];
    let mut bytes_returned = 0u32;

    unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some(&query as *const _ as *const _),
            std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            Some(buf.as_mut_ptr() as *mut _),
            buf.len() as u32,
            Some(&mut bytes_returned),
            None,
        )
        .ok()?;
    }

    let mut geometry = DISK_GEOMETRY_EX::default();
    let mut bytes_returned = 0u32;

    unsafe {
        DeviceIoControl(
            handle,
            IOCTL_DISK_GET_DRIVE_GEOMETRY_EX,
            None,
            0,
            Some(&mut geometry as *mut _ as *mut _),
            std::mem::size_of::<DISK_GEOMETRY_EX>() as u32,
            Some(&mut bytes_returned),
            None,
        )
        .ok()?;
    }

    let disk_size = geometry.DiskSize as f32 / 1_073_741_824f32;

    let desc = unsafe { &*(buf.as_ptr() as *const STORAGE_DEVICE_DESCRIPTOR) };

    // Vendor, product, serial are offsets into the same buffer
    let vendor = read_offset_str(&buf, desc.VendorIdOffset);
    let product = read_offset_str(&buf, desc.ProductIdOffset);
    //let serial = read_offset_str(&buf, desc.SerialNumberOffset);
    let is_usb = desc.BusType == BusTypeUsb;

    if is_usb {
        Some(format!(
            "{} {}{} | {:.2} GB",
            vendor.trim(),
            product.trim(),
            if is_usb { " USB Device" } else { "" },
            disk_size
        ))
    } else {
        None
    }
}

fn read_offset_str(buf: &[u8], offset: u32) -> String {
    if offset == 0 || offset as usize >= buf.len() {
        return String::new();
    }
    let slice = &buf[offset as usize..];
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    String::from_utf8_lossy(&slice[..end]).to_string()
}

pub(super) fn devices() -> Result<Vec<DeviceInfo>, String> {
    let mut list = vec![];
    for index in device_indexes() {
        let path = format!("\\\\.\\PhysicalDrive{}", index);
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        match unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                Default::default(),
                None,
            )
        } {
            Ok(handle) => {
                let info = if let Some(v) = get_drive_info(handle) {
                    v
                } else {
                    //println!("PhysicalDrive{index} should not be used");
                    continue;
                };
                //println!("PhysicalDrive{}: {}", index, info);
                unsafe { CloseHandle(handle).unwrap() };
                list.push(DeviceInfo {
                    loc: path,
                    name: info,
                })
            }
            Err(err) => {
                if err.code().0 == 0x80070005u32.cast_signed() {
                    return Err(format!(
                        "{err}\r\nMake sure to run this program with administrative privilages."
                    ));
                }
                eprintln!("{err}");
            }
        }
    }
    Ok(list)
}
