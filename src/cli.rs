use crate::dump;
use crate::facts::{Loan, Point, Region};
use crate::intern;
use crate::tab_delim;
use failure::Error;
use polonius_engine::{Algorithm, AllFacts, Output};
use std::path::Path;
use std::time::{Duration, Instant};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "borrow-check")]
pub struct Opt {
    #[structopt(
        short = "a",
        env = "POLONIUS_ALGORITHM",
        default_value = "naive",
        raw(possible_values = "&Algorithm::variants()", case_insensitive = "true")
    )]
    algorithm: Algorithm,
    #[structopt(long = "skip-tuples")]
    skip_tuples: bool,
    #[structopt(long = "skip-timing")]
    skip_timing: bool,
    #[structopt(short = "v")]
    verbose: bool,
    #[structopt(long = "graphviz_file")]
    graphviz_file: Option<String>,
    #[structopt(short = "o", long = "output")]
    output_directory: Option<String>,
    #[structopt(raw(required = "true"))]
    fact_dirs: Vec<String>,
}

pub fn main(opt: Opt) -> Result<(), Error> {
    try {
        let output_directory = opt.output_directory.map(|x| Path::new(&x).to_owned());
        let graphviz_file = opt.graphviz_file.map(|x| Path::new(&x).to_owned());
        for facts_dir in opt.fact_dirs {
            let tables = &mut intern::InternerTables::new();

            let result: Result<(Duration, AllFacts<Region, Loan, Point>, Output<Region, Loan, Point>), Error> = try {
                let verbose = opt.verbose;
                let all_facts =
                    tab_delim::load_tab_delimited_facts(tables, &Path::new(&facts_dir))?;
                let algorithm = opt.algorithm;
                let graphviz_output = graphviz_file.is_some();
                let (duration, output) =
                    timed(|| Output::compute(&all_facts, algorithm, verbose || graphviz_output));
                (duration, all_facts, output)
            };

            match result {
                Ok((duration, all_facts, output)) => {
                    println!("--------------------------------------------------");
                    println!("Directory: {}", facts_dir);
                    if !opt.skip_timing {
                        let seconds = duration.as_secs() as f64;
                        let millis = f64::from(duration.subsec_nanos()) * 0.000_000_001_f64;
                        println!("Time: {:0.3}s", seconds + millis);
                    }
                    if !opt.skip_tuples {
                        dump::dump_output(&output, &output_directory, tables)
                            .expect("Failed to write output");
                    }
                    if let Some(ref graphviz_file) = graphviz_file {
                        dump::graphviz(&output, &all_facts, graphviz_file, tables)
                            .expect("Failed to write GraphViz");
                    }
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
