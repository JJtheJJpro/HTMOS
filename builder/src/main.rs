mod os;

use std::{env, path::Path, process::ExitCode};
use sysinfo::System;

fn main() -> ExitCode {
    let mut sys = System::new_all();
    sys.refresh_all();
    println!("HTMOS Builder CLI 0.1.0");
    println!(
        "{} {}",
        System::name().unwrap(),
        System::os_version().unwrap()
    );
    println!();
    let args = env::args().collect::<Vec<String>>();
    let name = Path::new(&args[0]).file_name().unwrap().to_str().unwrap();
    let args = &args[1..];

    if args.len() == 0 {
        eprintln!("No arguments provided.  Type \"{name} ?\"");
        return ExitCode::from(1);
    }

    let mut i = 0;
    let mut move_on = true;
    let mut output = false;

    while move_on && i < args.len() {
        match args[i].as_str() {
            "?" => {
                move_on = false;

                println!("What an exciting time to be alive, isn't it?");
                println!();
                println!("-o [DEVICE#] : Output device");
                println!();
                println!("-l           : List devices");
                println!("");
            }
            "-l" => {
                #[cfg(windows)]
                if !os::is_elevated() {
                    #[cfg(windows)]
                    eprintln!("This program must be run under administrative privilages.");
                    //#[cfg(unix)]
                    //eprintln!("This program must be run as sudo.");
                    return ExitCode::from(2);
                }
                move_on = false;

                match os::devices() {
                    Ok(v) => {
                        for dev in v {
                            #[cfg(windows)]
                            println!("{}: {}", &dev.loc[4..], dev.name);
                            #[cfg(unix)]
                            println!("{}: {}", dev.loc, dev.name);
                        }
                        println!();
                        #[cfg(windows)]
                        println!("Specify the # in PhysicalDrive# for the output device.");
                        #[cfg(unix)]
                        println!("Specify the XXX in /dev/XXX for the output device.");
                    }
                    Err(e) => {
                        eprintln!("{e}");
                    }
                }
            }
            "-o" => {
                output = true;
                if i + 1 == args.len() {
                    eprintln!("Device not specified");
                    return ExitCode::from(4);
                }
            }
            arg => {
                if output {
                    output = false;
                    #[cfg(windows)]
                    let dev = if let Ok(_) = u32::from_str_radix(arg, 10) {
                        match os::devices() {
                            Ok(v) => {
                                let arg = String::from(arg);
                                let arg_copy = arg.clone();
                                if v.iter().any(move |i| {
                                    i.loc == format!("\\\\.\\PhysicalDrive{arg_copy}")
                                }) {
                                    format!("PhysicalDrive{arg}")
                                } else {
                                    eprintln!("Device not found");
                                    return ExitCode::from(5);
                                }
                            }
                            Err(e) => {
                                eprintln!("{e}");
                                return ExitCode::from(1);
                            }
                        }
                    } else {
                        eprintln!(
                            "Invalid device selection. \"{arg}\" is not a number, you fetcher (╯°□°)╯︵ ┻━┻"
                        );
                        return ExitCode::from(3);
                    };
                    #[cfg(unix)]
                    let dev = match os::devices() {
                        Ok(v) => {
                            let arg = String::from(arg);
                            let arg_copy = arg.clone();
                            if v.iter().any(move |i| i.loc == format!("/dev/{arg_copy}")) {
                                format!("/dev/{arg}")
                            } else {
                                eprintln!("Device not found");
                                return ExitCode::from(5);
                            }
                        }
                        Err(e) => {
                            eprintln!("{e}");
                            return ExitCode::from(1);
                        }
                    };

                    println!("Selected device: {dev}");
                }
            }
        }
        i += 1;
    }

    ExitCode::from(0)
}
