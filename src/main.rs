use omapaste::app;
use omapaste::cli::{self, Action};
use omapaste::paths::VERSION;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .init();
    match cli::parse_command(std::env::args().nth(1).as_deref()) {
        Ok(Action::Version) => {
            println!("omapaste {VERSION}");
        }
        Ok(Action::Help) => {
            println!("omapaste {VERSION}\n\nUsage: omapaste [daemon|toggle|show|hide|quit]\n");
        }
        Ok(Action::Run(command)) => {
            let code = app::run(&command);
            std::process::exit(code.into());
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    }
}
