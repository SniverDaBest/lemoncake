use core::fmt::{Write, Arguments};

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        if let Some(t) = $crate::TTY.lock().as_mut() {
            use core::fmt::Write;
            let _ = write!(t, "{}", format_args!($($arg)*));
        } else {
            $crate::serial_println!("No TTY available!");
        }
    };
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($fmt:expr) => ($crate::print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::print!(
        concat!($fmt, "\n"), $($arg)*));
}

#[doc(hidden)]
pub fn _print(args: Arguments) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        crate::serial::SERIAL1
            .lock()
            .write_fmt(args)
            .expect("Printing to serial failed");
    });
    #[allow(unused_must_use)]
    if let Some(t) = crate::TTY.lock().as_mut() {
        write!(t, "{}", args);
    }
}

#[macro_export]
macro_rules! all_print {
    ($($arg:tt)*) => {
        $crate::logging::_print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! all_println {
    () => ($crate::all_print!("\n"));
    ($fmt:expr) => ($crate::all_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::all_print!(
        concat!($fmt, "\n"), $($arg)*));
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::all_println!(
            "\x1b[34m{}[INFO ]:\x1b[0m {}",
            if cfg!(feature = "status-faces") { "(o_o) " } else { "" },
            format_args!($($arg)*)
        )
    };
}

#[macro_export]
macro_rules! warning {
    ($($arg:tt)*) => {
        $crate::all_println!(
            "\x1b[33m{}[WARN ]:\x1b[0m {} [{}@L{}:{}]",
            if cfg!(feature = "status-faces") { "(0_0) " } else { "" },
            format_args!($($arg)*),
            file!(),
            line!(),
            column!(),
        )
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::all_println!(
            "\x1b[31m{}[ERROR]:\x1b[0m {} [{}@L{}:{}]",
            if cfg!(feature = "status-faces") { "(X_X) " } else { "" },
            format_args!($($arg)*),
            file!(),
            line!(),
            column!(),
        )
    };
}

#[macro_export]
macro_rules! nftodo {
    ($($arg:tt)*) => {
        $crate::all_println!(
            "\x1b[35m{}[TODO ]:\x1b[0m {} [{}@L{}:{}]",
            if cfg!(feature = "status-faces") { "(-_-) " } else { "" },
            format_args!($($arg)*),
            file!(),
            line!(),
            column!(),
        )
    };
}