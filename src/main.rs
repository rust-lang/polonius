use polonius::cli;
use std::process::exit;

fn main() -> Result<(), cli::Error> {
    match cli::options_from_args() {
        Ok(options) => cli::main(options),
        Err(e) => {
            // override default `Termination` error printing
            eprintln!("{}\n\nFor more information try --help", e);
            exit(1);
        }
    }
}
