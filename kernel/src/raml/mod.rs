use libacpica::*;

pub mod consts;
pub mod types;

pub fn init_acpica() {
    unsafe {
        let status = AcpiInitializeTables(core::ptr::null_mut(), 16, false);
        if status != AE_OK {
            panic!("Failed to init tables");
        }

        let status = AcpiInitializeSubsystem();
        if status != AE_OK {
            panic!("Failed to init subsystem");
        }

        let status = AcpiLoadTables();
        if status != AE_OK {
            panic!("Failed to load AML tables");
        }
    }
}
