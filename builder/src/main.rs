use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write, stdin, stdout},
};

fn main() {
    print!("Enter file path > ");
    stdout().flush().unwrap();
    let mut fileinput = String::new();
    if let Ok(_) = stdin().read_line(&mut fileinput) {
        let mut f = match File::open(fileinput.trim()) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error opening file: {e}");
                return;
            }
        };

        let mut buffer = Vec::new();
        if let Err(e) = f.read_to_end(&mut buffer) {
            eprintln!("Error reading file: {e}");
            return;
        }

        if buffer[510] != 0x55 || buffer[511] != 0xAA {
            println!("WARNING: 0xAA55 signature not found at end of file.");
        }

        let mut device = match OpenOptions::new().write(true).open("/dev/sda") {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Error accessing device with write permissions: {e}");
                return;
            }
        };

        if let Err(e) = device.seek(SeekFrom::Start(0)) {
            eprintln!("Error seeking device: {e}");
            return;
        }
        if let Err(e) = device.write_all(&buffer) {
            eprintln!("Error writing to device: {e}");
            return;
        }

        if let Err(e) = device.sync_all() {
            eprintln!("Error syncing device: {e}");
            return;
        }

        println!("Successfully wrote {} bytes to /dev/sda", buffer.len());
    } else {
        eprintln!("No file selected.");
    }
}
