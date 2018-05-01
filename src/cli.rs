use crate::intern;
use crate::output::Output;
use crate::tab_delim;
use failure::Error;
use std::env;
use std::path::Path;
use std::time::{Duration, Instant};

pub fn main() -> Result<(), Error> {
    do catch {
        for facts_dir in env::args().skip(1) {
            let tables = &mut intern::InternerTables::new();

            let result: Result<(Duration, Output), Error> = do catch {
                let all_facts =
                    tab_delim::load_tab_delimited_facts(tables, Path::new(&facts_dir))?;
                timed(|| Output::compute(all_facts, false))
            };

            match result {
                Ok((duration, output)) => {
                    println!("--------------------------------------------------");
                    println!("Directory: {}", facts_dir);
                    let seconds: f64 = duration.as_secs() as f64;
                    let millis: f64 = duration.subsec_nanos() as f64 * 0.000_000_001_f64;
                    println!("Time: {:0.3}s", seconds + millis);
                    output.dump(tables);
                }

                Err(error) => {
                    eprintln!("`{}`: {}", facts_dir, error);
                }
            }
        }
    }
}

fn timed<T>(op: impl FnOnce() -> T) -> (Duration, T) {
    let start = Instant::now();
    let output = op();
    let duration = start.elapsed();
    (duration, output)
}
