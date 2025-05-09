use crate::{info, keyboard::ScancodeStream, print, println};
use alloc::string::String;
use futures_util::StreamExt;
use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts::Us104Key};

pub async fn run_command_line() {
    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(ScancodeSet1::new(), Us104Key, HandleControl::Ignore);

    let mut input_buffer = String::new();
    let prompt = "> ";

    print!("{}", prompt);

    loop {
        while let Some(scancode) = scancodes.next().await {
            info!("Scancode: {}", scancode);
            if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
                if let Some(key) = keyboard.process_keyevent(key_event) {
                    match key {
                        DecodedKey::Unicode(character) => {
                            if character == '\n' {
                                print!("\n");
                                process_command(&input_buffer);
                                input_buffer.clear();

                                print!("{}", prompt);
                            } else {
                                input_buffer.push(character);
                                print!("{}", character);
                            }
                        }
                        DecodedKey::RawKey(_) => {}
                    }
                }
            }
        }
    }
}

fn process_command(buf: &String) {
    match buf.as_str() {
        "cat" => info!("meow :3"),
        x => println!("{} is not a known command.", x),
    }
}
