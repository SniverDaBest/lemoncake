use crate::{
    LEMONCAKE_VER, base64,
    disks::{self, ahci},
    error, info, keyboard, nftodo,
    pci::{self, scan_pci_bus},
    print, println,
    vga::{Color, WRITER, set_bg, set_fg},
    warning,
};
use alloc::{
    string::{String, ToString},
    vec::*,
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
    if command.trim().starts_with("echo") {
        println!("{}", command.replace("echo ", "").as_str().trim());
    } else if command.trim().starts_with("clear") {
        WRITER.lock().clear_screen();
    } else if command.trim().starts_with("ver") {
        println!(
            "SHSH Version {}\nLemoncake version: {}",
            SHSH_VERSION, LEMONCAKE_VER
        );
    } else if command.trim().starts_with("b64encode") {
        let input_str = command.split_whitespace().nth(1).unwrap_or("").as_bytes();
        println!("{}", base64::encode(input_str));
    } else if command.trim().starts_with("b64decode") {
        let input_str = command.split_whitespace().nth(1).unwrap_or("").as_bytes();
        println!("{}", base64::decode(input_str));
    } else if command.trim().starts_with("color") {
        let cmds: Vec<&str> = command.trim().split_ascii_whitespace().collect();
        if cmds.len() < 3 {
            println!("Usage:");
            println!("  [foreground color] [background color]");
            return;
        }

        let fg: &str = cmds[1];
        match fg.to_lowercase().as_str() {
            "red" => set_fg(Color::Red),
            "lightred" => set_fg(Color::LightRed),
            "yellow" => set_fg(Color::Yellow),
            "green" => set_fg(Color::Green),
            "lightgreen" => set_fg(Color::LightGreen),
            "cyan" => set_fg(Color::Cyan),
            "lightcyan" => set_fg(Color::LightCyan),
            "blue" => set_fg(Color::Blue),
            "lightblue" => set_fg(Color::LightBlue),
            "magenta" => set_fg(Color::Magenta),
            "pink" => set_fg(Color::Pink),
            "white" => set_fg(Color::White),
            "darkgray" => set_fg(Color::DarkGray),
            "lightgray" => set_fg(Color::LightGray),
            "brown" => set_fg(Color::Brown),
            "black" => set_fg(Color::Black),
            c => {
                warning!("Color {} is not an acceptable value!", c);
            }
        }

        let bg: &str = cmds[2];
        match bg.to_lowercase().as_str() {
            "red" => set_bg(Color::Red),
            "lightred" => set_bg(Color::LightRed),
            "yellow" => set_bg(Color::Yellow),
            "green" => set_bg(Color::Green),
            "lightgreen" => set_bg(Color::LightGreen),
            "cyan" => set_bg(Color::Cyan),
            "lightcyan" => set_bg(Color::LightCyan),
            "blue" => set_bg(Color::Blue),
            "lightblue" => set_bg(Color::LightBlue),
            "magenta" => set_bg(Color::Magenta),
            "pink" => set_bg(Color::Pink),
            "white" => set_bg(Color::White),
            "darkgray" => set_bg(Color::DarkGray),
            "lightgray" => set_bg(Color::LightGray),
            "brown" => set_bg(Color::Brown),
            "black" => set_bg(Color::Black),
            c => {
                warning!("Color {} is not an acceptable value!", c);
            }
        }
    } else if command.trim().starts_with("disks") {
        let devs = ahci::scan_for_ahci_devs();
        for dev in devs {
            info!("Found AHCI Device: {}", dev);
        }
    } else if command.trim().starts_with("pci") {
        let cmds: Vec<&str> = command.trim().split_ascii_whitespace().collect();
        if cmds.len() < 2 {
            println!("Usage:");
            println!("  -h -- Shows this message");
            println!("  -s -- Searches for all PCI devices.");
            return;
        }

        if cmds[1] == "-s" {
            let pci_devs = unsafe { scan_pci_bus() };
            for dev in pci_devs {
                info!("Got PCI Device: {}", dev);
            }
        } else if cmds[1] == "-h" {
            println!("Usage:");
            println!("  -h -- Shows this message");
            println!("  -s -- Searches for all PCI devices.");
        }
    } else if command.trim().starts_with("error") {
        error!("{}", command.replace("error", "").as_str().trim());
    } else if command.trim().starts_with("warning") {
        warning!("{}", command.replace("warning", "").as_str().trim());
    } else if command.trim().starts_with("info") {
        info!("{}", command.replace("info", "").as_str().trim());
    } else if command.trim().starts_with("panic") {
        panic!("{}", command.replace("panic", "").as_str().trim());
    } else if command.trim().starts_with("help") {
        println!("SHSH Version {}.", SHSH_VERSION);
        println!("help -- Shows this message.");
        println!("echo [input] -- Echos user input.");
        println!("clear -- Clears the screen.");
        println!(
            "ver -- Shows the version of SHSH and Lemoncake. (currently running SHSH {})",
            SHSH_VERSION
        );
        println!("color [fg, bg] -- Changes the color");
        println!("b64encode [input] -- Encodes user input into Base64");
        println!("b64decode [base64] -- Decodes Base64 user input into normal text.");
        println!("disks -- The disk utility.");
        println!("pci -- The PCI utility");
        println!("error [message] -- prints like an error");
        println!("warning [message] -- prints like a warning");
        println!("info [message] -- prints like info");
        println!("panic [message] -- panics the system\n");
    } else if command.trim() == "" {
        println!();
    } else {
        println!("Unknown command: {}", command);
    }
}
