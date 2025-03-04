use crate::{LEMONCAKE_VER, acpi, base64, keyboard, nftodo, print, println, vga::WRITER};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use futures_util::stream::StreamExt;
use pc_keyboard::{DecodedKey, Keyboard, ScancodeSet1};

static SHSH_VERSION: &str = "b0.5";

pub async fn run_command_line() {
    println!("Made by SniverDaBest\nSHSH {}", SHSH_VERSION);
    let mut scancodes = keyboard::ScancodeStream::new();
    let mut keyboard = Keyboard::new(
        ScancodeSet1::new(),
        pc_keyboard::layouts::Us104Key,
        pc_keyboard::HandleControl::Ignore,
    );

    let mut input_buffer = String::new();
    let mut prompt = "> ".to_string();

    // Initial prompt display
    print!("{}", prompt);

    loop {
        while let Some(scancode) = scancodes.next().await {
            if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
                if let Some(key) = keyboard.process_keyevent(key_event) {
                    match key {
                        DecodedKey::Unicode(character) => {
                            if character == '\n' {
                                print!("\n");
                                // Process command
                                process_command(&input_buffer);
                                input_buffer.clear();
                                prompt = "> ".to_string();
                                // Move to the next line and show the new prompt

                                print!("{}", prompt);
                            } else {
                                input_buffer.push(character);
                                // Redraw input buffer
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

fn process_command(command: &str) {
    if command.trim().contains("echo") {
        println!("{}", command.replace("echo ", ""));
    } else if command.trim().contains("clear") {
        WRITER.lock().clear_screen();
    } else if command.trim().contains("ver") {
        println!(
            "SHSH Version {}\nLemoncake version: {}",
            SHSH_VERSION, LEMONCAKE_VER
        );
    } else if command.trim().contains("b64encode") {
        let input_str = command.split_whitespace().nth(1).unwrap_or("").as_bytes();
        println!("{}", base64::encode(input_str));
    } else if command.trim().contains("b64decode") {
        let input_str = command.split_whitespace().nth(1).unwrap_or("").as_bytes();
        println!("{}", base64::decode(input_str));
    } else if command.trim().contains("color") {
        nftodo!();
    } else if command.trim().contains("help") {
        println!("SHSH Version {}.", SHSH_VERSION);
        println!("help -- Shows this message.");
        println!("echo [input] -- Echos user input.");
        println!("clear -- Clears the screen.");
        println!(
            "ver -- Shows the version of SHSH and Lemonade. (currently running version {})",
            SHSH_VERSION
        );
        println!("color -- Changes the color");
        println!("b64encode [input] -- Encodes user input into Base64");
        println!("b64decode [base64] -- Decodes Base64 user input into normal text.");
    } else if command.trim() == "" {
        println!();
    } else {
        println!("Unknown command: {}", command);
    }
}
