use std::env;
use std::process;

fn main() {
    let cwd = match env::current_dir() {
        Ok(cwd) => cwd,
        Err(err) => {
            eprintln!("gitcrack: {err}");
            process::exit(1);
        }
    };
    if let Err(err) = gitcrack::app::run(cwd) {
        eprintln!("gitcrack: {err:#}");
        process::exit(1);
    }
}
