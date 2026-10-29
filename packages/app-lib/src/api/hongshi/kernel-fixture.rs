use std::io::{self, Write};
use std::time::Duration;

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--print-help") {
        let directory = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let count = directory.join("help-count");
        let previous = std::fs::read_to_string(&count)
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        std::fs::write(count, (previous + 1).to_string()).unwrap();
        if directory.join("slow-help").exists() {
            std::fs::write(
                directory.join("help-started"),
                std::process::id().to_string(),
            )
            .unwrap();
            std::thread::sleep(Duration::from_secs(30));
        }
        println!("--to --game-port --game-host --control-port --data-port");
        return;
    }
    let node = args
        .windows(2)
        .find(|pair| pair[0] == "-t")
        .map(|pair| pair[1].as_str())
        .unwrap_or("normal.example.com");
    if node == "error.example.com" {
        std::process::exit(1);
    }
    if node == "empty-close.example.com" {
        return;
    }
    if node == "abnormal.example.com" {
        std::process::exit(2);
    }
    if node == "timeout.example.com" {
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    if node == "noisy.example.com" {
        for _ in 0..2000 {
            eprintln!("{}", "stderr stays drained ".repeat(64));
        }
    }
    if node == "invalid.example.com" {
        println!("endpoint=relay.example.com:0");
    } else {
        println!(
            "\x1b[32mINFO arbitrary decoration\x1b[0m endpoint=relay.example.com:34575"
        );
    }
    io::stdout().flush().unwrap();
    if node == "double.example.com" {
        println!("endpoint=other.example.com:34576");
        io::stdout().flush().unwrap();
    }
    if node == "immediate.example.com" {
        return;
    }
    if node == "closed.example.com" {
        std::thread::sleep(Duration::from_millis(100));
        return;
    }
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}
