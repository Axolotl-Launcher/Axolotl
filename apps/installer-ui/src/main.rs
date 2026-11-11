#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod executable_install;
#[cfg(windows)]
mod prerequisite;
#[cfg(windows)]
mod windows;

#[cfg(windows)]
fn main() {
    if let Some(result) =
        executable_install::dispatch(std::env::args_os().skip(1))
    {
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(2);
        }
        return;
    }
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
