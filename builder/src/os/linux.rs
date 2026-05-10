use crate::os::DeviceInfo;

fn get_device_info(dev: &str) -> String {
    // dev is e.g. "/dev/sdb" — extract just "sdb"
    let name = dev.trim_start_matches("/dev/");
    let base = format!("/sys/block/{}", name);

    let model = read_sys(&format!("{}/device/model", base));
    let vendor = read_sys(&format!("{}/device/vendor", base));
    let size_sectors = read_sys(&format!("{}/size", base))
        .trim()
        .parse::<u64>()
        .unwrap_or(0);
    let size_gb = (size_sectors * 512) as f32 / 1_073_741_824f32;

    // removable: 1 = removable (USB), 0 = internal
    //let removable = read_sys(&format!("{}/removable", base)).trim().to_string();

    // rotation: 0 = SSD/NVMe, 1 = HDD, missing = unknown
    let rotational = read_sys(&format!("{}/queue/rotational", base))
        .trim()
        .to_string();

    let media_type = match rotational.as_str() {
        "1" => " USB Device",
        _ => "",
    };

    format!(
        "{} {}{} | {:.2} GB",
        vendor.trim(),
        model.trim(),
        media_type,
        size_gb,
    )
}

fn read_sys(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn enumerate_block_devices() -> Vec<String> {
    let Ok(entries) = std::fs::read_dir("/sys/block") else {
        return Vec::new();
    };

    entries
        .filter_map(|e| e.ok())
        .map(|e| format!("/dev/{}", e.file_name().to_string_lossy()))
        .filter(|path| {
            // Filter out loop devices, ram disks, etc.
            let name = path.trim_start_matches("/dev/");
            !name.starts_with("loop")
                && !name.starts_with("ram")
                && !name.starts_with("zram")
                && !name.starts_with("nvme")
        })
        .collect()
}

pub(super) fn devices() -> Result<Vec<DeviceInfo>, String> {
    let mut devs = vec![];

    for dev in enumerate_block_devices() {
        let info = get_device_info(dev.as_str());
        devs.push(DeviceInfo {
            loc: dev,
            name: info,
        });
    }

    Ok(devs)
}
