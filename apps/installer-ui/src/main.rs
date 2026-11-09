#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod prerequisite;
#[cfg(windows)]
mod windows;

#[cfg(windows)]
fn main() {
    if let Some(chinese) = prerequisite::requested_language() {
        std::process::exit(prerequisite::show(chinese));
    }
    if let Err(error) = windows::run() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("axolotl-installer-ui is only available on Windows");
}
