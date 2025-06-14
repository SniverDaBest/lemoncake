use crate::{clear, error, keyboard::ScancodeStream, print, println, sad, warning, yay};
use alloc::{format, string::*, sync::Arc, vec, vec::*};
use futures_util::StreamExt;
use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};
use spin::Mutex;
use spinning_top::Spinlock;
use x86_64::instructions::port::Port;

pub static COMMAND_REGISTRY: Spinlock<Option<CommandRegistry>> = Spinlock::new(None);

#[derive(Debug)]
pub struct Command {
    name: &'static str,
    aliases: Vec<&'static str>,
    help_message: &'static str,
    func: fn(&CommandRegistry, Vec<&str>) -> i32,
}

impl Command {
    pub fn new(
        name: &'static str,
        aliases: Vec<&'static str>,
        help_message: &'static str,
        func: fn(&CommandRegistry, Vec<&str>) -> i32,
    ) -> Self {
        return Self {
            name,
            aliases,
            help_message,
            func,
        };
    }

    pub fn get_name(&self) -> &'static str {
        return self.name;
    }

    pub fn get_aliases(&self) -> Vec<&'static str> {
        return self.aliases.clone();
    }

    pub fn get_help_msg(&self) -> &'static str {
        return self.help_message;
    }
}

#[derive(Debug)]
pub struct CommandRegistry {
    commands: Vec<Command>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        return CommandRegistry {
            commands: Vec::new(),
        };
    }

    pub fn push(&mut self, cmd: Command) {
        self.commands.push(cmd);
    }

    pub fn pop(&mut self) -> Option<Command> {
        return self.commands.pop();
    }

    pub fn get_help(&self) -> String {
        let mut s = String::new();

        s.push_str("=| Help |===============\n");

        for cmd in self.commands.iter() {
            s.push_str(format!("{} | {}\n", cmd.get_name(), cmd.get_help_msg()).as_str());
        }

        return s;
    }

    pub fn print_help(&self) {
        println!("{}", self.get_help());
    }

    pub fn search(&self, name: &str) -> Option<&Command> {
        for cmd in self.commands.iter() {
            if cmd.name == name {
                return Some(cmd);
            } else if cmd.aliases.contains(&name) {
                return Some(cmd);
            }
        }

        return None;
    }

    pub fn exec_command(&self, input: Vec<&str>) -> Option<i32> {
        if input.is_empty() {
            return None;
        }
        let cmd = self.search(input[0]);
        if cmd.is_none() {
            return None;
        }
        return Some((cmd.unwrap().func)(self, input[1..].to_vec()));
    }
}

fn license(_registry: &CommandRegistry, _args: Vec<&str>) -> i32 {
    println!(
        "Lemoncake is licensed under the 2-Clause (simplified) BSD License\n(c) SniverDaBest 2025"
    );
    return 0;
}

fn credits(_registry: &CommandRegistry, _args: Vec<&str>) -> i32 {
    println!(
        "Lemoncake is developed by SniverDaBest, and uses some external crates/libraries, most of which are developed by the Rust OSDev community.\nIt also uses some code from Ruddle/Fomos on GitHub for the APIC and IOAPIC."
    );
    return 0;
}

fn smiley(_registry: &CommandRegistry, _args: Vec<&str>) -> i32 {
    yay!();
    println!();
    return 0;
}

fn sad(_registry: &CommandRegistry, _args: Vec<&str>) -> i32 {
    sad!();
    println!();
    return 0;
}

fn clear(_registry: &CommandRegistry, _args: Vec<&str>) -> i32 {
    clear!();
    return 0;
}

fn whoami(_registry: &CommandRegistry, _args: Vec<&str>) -> i32 {
    println!(
        "You're the user... why would you need to ask who you are? I feel like you should be able to figure that out by yourself. If you can't, then go see a doctor, or maybe go to the ER."
    );
    return 0;
}

fn help(registry: &CommandRegistry, _args: Vec<&str>) -> i32 {
    registry.print_help();
    return 0;
}

fn shutdown(_registry: &CommandRegistry, args: Vec<&str>) -> i32 {
    if args.len() != 1 {
        println!(
            "Usage:\n  shutdown <vm_type>\n  poweroff <vm_type>\nExamples:\n  shutdown qemu\n  shutdown bochs\n  poweroff vbox\n  poweroff old_qemu\nNote: old_qemu is the same as bochs."
        );
        return 1;
    }

    unsafe {
        match args[0].to_lowercase().as_str() {
            "qemu" => Port::new(0x604).write(0x2000 as u32),
            "bochs" | "old_qemu" => Port::new(0xB004).write(0x2000 as u32),
            "vbox" | "virtualbox" => Port::new(0x4004).write(0x3400 as u32),
            t => println!(
                "Type \"{}\" is invalid!\nUsage:\n  shutdown <vm_type>\n  poweroff <vm_type>\nExamples:\n  shutdown qemu\n  shutdown bochs\n  poweroff vbox\n  poweroff old_qemu\nNote: old_qemu is the same as bochs.",
                t
            ),
        }
    }

    return 0;
}

fn panic_(_registry: &CommandRegistry, _args: Vec<&str>) -> i32 {
    panic!("Panic initiated from command line.");
}

fn init_command_registry() -> CommandRegistry {
    let license_cmd = Command::new(
        "license",
        vec!["lic", "l"],
        "Displays the Lemoncake license and copyright information.",
        license,
    );
    let credits_cmd = Command::new(
        "credits",
        vec!["cred", "c"],
        "Displays credits to the contributer(s) of Lemoncake.",
        credits,
    );
    let smiley_cmd = Command::new(
        "smiley",
        vec!["yay", ":D", ":)"],
        "Prints a smiley face :)",
        smiley,
    );
    let sad_cmd = Command::new("sad", vec!["nay", "D:", ":("], "Prints a sad face :(", sad);
    let clear_cmd = Command::new(
        "clear",
        vec!["claer", "clera", "cear"],
        "Clears the TTY.",
        clear,
    );
    let whoami_cmd = Command::new("whoami", vec![], "Tells you who you are.", whoami);
    let panic_cmd = Command::new("panic", vec![], "Panics the system.", panic_);
    let shutdown_cmd = Command::new(
        "shutdown",
        vec!["poweroff"],
        "Powers off the VM. (does not work on real hw!)",
        shutdown,
    );
    let help_cmd = Command::new(
        "help",
        vec!["hlep", "h", "?"],
        "Displays this message.",
        help,
    );

    let mut reg = CommandRegistry::new();
    reg.push(license_cmd);
    reg.push(credits_cmd);
    reg.push(smiley_cmd);
    reg.push(sad_cmd);
    reg.push(clear_cmd);
    reg.push(whoami_cmd);
    reg.push(panic_cmd);
    reg.push(shutdown_cmd);
    reg.push(help_cmd);

    return reg;
}

pub async fn run_command_line(scancodes: Arc<Mutex<ScancodeStream>>) {
    let mut input_buffer = String::new();
    let mut prev_ret_code = 0;
    let mut scs = scancodes.lock();

    let mut keyboard = Keyboard::new(
        ScancodeSet1::new(),
        layouts::Us104Key,
        HandleControl::Ignore,
    );

    if COMMAND_REGISTRY.lock().is_some() {
        warning!("Command registry already exists. Not replacing it...");
    } else {
        *COMMAND_REGISTRY.lock() = Some(init_command_registry());
    }

    print!("[{}] > ", prev_ret_code);

    loop {
        while let Some(scancode) = scs.next().await {
            if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
                if let Some(key) = keyboard.process_keyevent(key_event) {
                    match key {
                        DecodedKey::Unicode(character) => match character {
                            '\u{7f}' => {
                                input_buffer.pop();
                            }
                            '\n' => {
                                println!();
                                prev_ret_code = process_command(&input_buffer, prev_ret_code);
                                input_buffer.clear();
                                print!("[{}] > ", prev_ret_code);
                            }
                            c => {
                                print!("{}", c);
                                input_buffer.push(c);
                            }
                        },
                        DecodedKey::RawKey(_) => {}
                    }
                }
            }
        }
    }
}

fn process_command(buf: &String, prev_ret_code: i32) -> i32 {
    if buf.trim().starts_with("#") || buf.trim().is_empty() {
        return prev_ret_code;
    }

    if COMMAND_REGISTRY.lock().is_none() {
        error!("Command registry is uninitialized!");
        return -2;
    }

    let data: Vec<&str> = buf.trim().split(' ').collect();

    let cmd_name = data.get(0).copied().unwrap_or("");

    if let Some(registry) = COMMAND_REGISTRY.lock().as_ref() {
        let x = registry.exec_command(data);
        if x.is_none() {
            println!("Command '{}' not found.", cmd_name);
            return -1;
        } else {
            return x.unwrap();
        }
    } else {
        error!("Unable to lock and take access of the command registry!");
        return -3;
    }
}
