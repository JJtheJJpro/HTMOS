//! JJ's Firmware Table Union Type Declarations

use core::{ffi::c_void, fmt::Arguments};
use r_efi::efi::Guid;

use crate::println;

// just from my laptop for now.
// EE4E5898-3914-4259-9D6E-DC7BD79403CF (LZMACustomDecompress)
// 05AD34BA-6F02-4214-952E-4DA0398E2BB9
// 7739F24C-93D7-11D4-9A3A-0090273FC14D
// 4C19049F-4137-4DD3-9C10-8B97A83FFDFA
// 49152E77-1ADA-4764-B7A2-7AFEFED95E8B
// 00781CA1-5DE3-405F-ABB8-379C3C076984
// EB9D2D30-2D88-11D3-9A16-0090273FC14D
// 8868E871-E4F1-11D3-BC22-0080C73C8881
// 1E2ED096-30E2-4254-BD89-863BBEF82325
// 4E28CA50-D582-44AC-A11F-E3D56526DB34
// EB9D2D31-2D88-11D3-9A16-0090273FC14D
// F2FD1544-9794-4A2C-992E-E5BBCF20E394
// DCFA911D-26EB-469F-A220-38B7DC461220
// B122A263-3661-4F68-9929-78F8B0D62180

//let vguid = cfg.vendor_guid.as_fields();
//println!(
//    "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}  0x{:16X}",
//    vguid.0,
//    vguid.1,
//    vguid.2,
//    vguid.3,
//    vguid.4,
//    vguid.5[0],
//    vguid.5[1],
//    vguid.5[2],
//    vguid.5[3],
//    vguid.5[4],
//    vguid.5[5],
//    cfg.vendor_table as usize
//);

const fn u8_hex_str_value_upper(v: u8) -> (u8, u8) {
    (
        match v >> 4 {
            0x0 => b'0',
            0x1 => b'1',
            0x2 => b'2',
            0x3 => b'3',
            0x4 => b'4',
            0x5 => b'5',
            0x6 => b'6',
            0x7 => b'7',
            0x8 => b'8',
            0x9 => b'9',
            0xA => b'A',
            0xB => b'B',
            0xC => b'C',
            0xD => b'D',
            0xE => b'E',
            0xF => b'F',
            _ => unreachable!(),
        },
        match v & 0xF {
            0x0 => b'0',
            0x1 => b'1',
            0x2 => b'2',
            0x3 => b'3',
            0x4 => b'4',
            0x5 => b'5',
            0x6 => b'6',
            0x7 => b'7',
            0x8 => b'8',
            0x9 => b'9',
            0xA => b'A',
            0xB => b'B',
            0xC => b'C',
            0xD => b'D',
            0xE => b'E',
            0xF => b'F',
            _ => unreachable!(),
        },
    )
}

//pub const fn guid_str_lower(guid: Guid) -> &'static str {}
pub const fn guid_utf8_upper(guid: Guid) -> [u8; 36] {
    let vguid = guid.as_fields();

    let tl1 = u8_hex_str_value_upper((vguid.0 >> 24) as u8);
    let tl2 = u8_hex_str_value_upper(((vguid.0 >> 16) & 0xFF) as u8);
    let tl3 = u8_hex_str_value_upper(((vguid.0 >> 8) & 0xFF) as u8);
    let tl4 = u8_hex_str_value_upper((vguid.0 & 0xFF) as u8);

    let tm1 = u8_hex_str_value_upper((vguid.1 >> 8) as u8);
    let tm2 = u8_hex_str_value_upper((vguid.1 & 0xFF) as u8);

    let th1 = u8_hex_str_value_upper((vguid.2 >> 8) as u8);
    let th2 = u8_hex_str_value_upper((vguid.2 & 0xFF) as u8);

    let csh = u8_hex_str_value_upper(vguid.3);
    let csl = u8_hex_str_value_upper(vguid.4);

    let n1 = u8_hex_str_value_upper(vguid.5[0]);
    let n2 = u8_hex_str_value_upper(vguid.5[1]);
    let n3 = u8_hex_str_value_upper(vguid.5[2]);
    let n4 = u8_hex_str_value_upper(vguid.5[3]);
    let n5 = u8_hex_str_value_upper(vguid.5[4]);
    let n6 = u8_hex_str_value_upper(vguid.5[5]);
    [
        tl1.0, tl1.1, tl2.0, tl2.1, tl3.0, tl3.1, tl4.0, tl4.1, b'-', tm1.0, tm1.1, tm2.0, tm2.1,
        b'-', th1.0, th1.1, th2.0, th2.1, b'-', csh.0, csh.1, csl.0, csl.1, b'-', n1.0, n1.1, n2.0,
        n2.1, n3.0, n3.1, n4.0, n4.1, n5.0, n5.1, n6.0, n6.1,
    ]
}

/// LZMA Custom Decompress
pub const LZMA_CUSTOM_DECOMPRESS: Guid = Guid::from_fields(
    0xEE4E5898,
    0x3914,
    0x4259,
    0x9D,
    0x6E,
    &[0xDC, 0x7B, 0xD7, 0x94, 0x03, 0xCF],
);

#[repr(C)]
pub struct LZMACustomDecompress {
    pub guid: Guid,
    pub compressed_data: &'static [u8],
}

pub enum FirmwareTable {
    LZMACustomDecompress(&'static LZMACustomDecompress),
}
impl FirmwareTable {
    #[must_use]
    pub fn parse(guid: Guid, ptr: *mut c_void) -> Result<Self, ()> {
        match guid {
            LZMA_CUSTOM_DECOMPRESS => Ok(FirmwareTable::LZMACustomDecompress(unsafe {
                &*(ptr as *const _ as *const LZMACustomDecompress)
            })),
            _ => {
                println!(
                    "GUID {} not implemented; kernel will continue to run.",
                    str::from_utf8(&guid_utf8_upper(guid)).unwrap()
                );
                println!(
                    "Visit 'https://jjthejjpro.com/' to send this GUID to help out with this project."
                );
                Err(())
            }
        }
    }
}
