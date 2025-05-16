use alloc::{string::String, vec::Vec};
use uefi::{boot, prelude::*, print, println, proto::console::text::Key, Error, Result};

mod stdin;
mod stdout;

pub fn clear() -> core::result::Result<(), Error> {
    stdout::clear()
}

/// Reads a line of UTF-16 console input, echoing it, handling Backspace,
/// and terminating on Enter. Returns a UTF-8 `String`.
pub fn readline() -> Result<String> {
    let mut buf = Vec::new();

    loop {
        if let Some(event) = stdin::wait_for_key_event() {
            match boot::wait_for_event(&mut [event]) {
                Ok(_v) => match stdin::read_key() {
                    Ok(pk) => {
                        if let Some(key) = pk {
                            match key {
                                Key::Printable(c) => {
                                    if c == '\r' || c == '\n' {
                                        println!();
                                        break;
                                    } else if c == '\u{8}' {
                                        if buf.pop().is_some() {
                                            print!("{c} {c}");
                                        }
                                        continue;
                                    }
                                    buf.push(c.into());
                                    print!("{c}");
                                }
                                Key::Special(sp) => {
                                    print!("{sp:?}");
                                }
                            }
                        }
                    }
                    Err(e) => return Err(e),
                },
                Err(e) => {
                    if let Some(ev) = e.data() {
                        return Err(Error::new(Status(*ev), ()));
                    } else {
                        return Err(Error::new(Status::UNSUPPORTED, ()));
                    }
                }
            }
        } else {
            return Err(Error::new(Status::NOT_READY, ()));
        }
    }

    Ok(String::from_utf16_lossy(&buf))
}
