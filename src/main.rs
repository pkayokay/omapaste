use omapaste::app;
use omapaste::paths::VERSION;

const COMMANDS: &[&str] = &["daemon", "start", "toggle", "show", "hide", "quit", "stop"];

fn parse_command() -> String {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--version") | Some("-V") => {
            println!("omapaste {VERSION}");
            std::process::exit(0);
        }
        Some("--help") | Some("-h") => {
            println!("omapaste {VERSION}\n\nUsage: omapaste [daemon|toggle|show|hide|quit]\n");
            std::process::exit(0);
        }
        Some(cmd) if COMMANDS.contains(&cmd) => cmd.to_string(),
        Some(other) => {
            eprintln!("unknown command: {other}");
            std::process::exit(2);
        }
        None => "toggle".into(),
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .init();
    let command = parse_command();
    let code = app::run(&command);
    std::process::exit(code.into());
}
