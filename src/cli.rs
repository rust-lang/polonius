use crate::intern;
use crate::output::Output;
use crate::tab_delim;
use failure::Error;
use std::time::{Duration, Instant};

use std::path::PathBuf;

#[derive(StructOpt, Debug)]
#[structopt(name = "borrow-check")]
pub struct Opt {
    #[structopt(long = "skip-tuples")]
    skip_tuples: bool,
    #[structopt(long = "skip-timing")]
    skip_timing: bool,
    #[structopt(raw(required = "true"))]
    fact_dirs: Vec<PathBuf>,
}

pub fn main(opt: Opt) -> Result<(), Error> {
    do catch {
        for facts_dir in opt.fact_dirs {
            let tables = &mut intern::InternerTables::new();

            let result: Result<(Duration, Output), Error> = do catch {
                let all_facts = tab_delim::load_tab_delimited_facts(tables, &facts_dir)?;
                timed(|| Output::compute(all_facts, false))
            };

            match result {
                Ok((duration, output)) => {
                    println!("--------------------------------------------------");
                    println!("Directory: {}", facts_dir.display());
                    if !opt.skip_timing {
                        let seconds: f64 = duration.as_secs() as f64;
                        let millis: f64 = duration.subsec_nanos() as f64 * 0.000_000_001_f64;
                        println!("Time: {:0.3}s", seconds + millis);
                    }
                    if !opt.skip_tuples {
                        output.dump(tables);
                    }
                }

                Err(error) => {
                    eprintln!("`{}`: {}", facts_dir.display(), error);
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
