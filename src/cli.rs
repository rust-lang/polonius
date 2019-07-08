use crate::dump;
use crate::facts::{Loan, Point, Region, Variable};
use crate::intern;
use crate::tab_delim;
use failure::Error;
use log::error;
use polonius_engine::{Algorithm, AllFacts, Output};
use std::path::Path;
use std::time::{Duration, Instant};
use structopt::StructOpt;

type PoloniusFacts = AllFacts<Region, Loan, Point, Variable>;
type PoloniusOutput = Output<Region, Loan, Point, Variable>;

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
    #[structopt(long = "show-tuples", help = "Show output tuples on stdout")]
    show_tuples: bool,
    #[structopt(long = "skip-timing", help = "Do not display timing results")]
    skip_timing: bool,
    #[structopt(
        short = "v",
        long = "verbose",
        help = "Show intermediate output tuples and not just errors"
    )]
    verbose: bool,
    #[structopt(
        long = "graphviz_file",
        help = "Generate a graphviz file to visualize the computation"
    )]
    graphviz_file: Option<String>,
    #[structopt(
        short = "o",
        long = "output",
        help = "Directory where to output resulting tuples"
    )]
    output_directory: Option<String>,
    #[structopt(raw(required = "true"))]
    fact_dirs: Vec<String>,
    #[structopt(
        long = "dump-liveness-graph",
        help = "Generate a graphviz file to visualize the liveness information"
    )]
    liveness_graph_file: Option<String>,

    #[structopt(
        long = "ignore-region-live-at",
        help = "ignore any provided region-live-at and let Polonius perform the calculation"
    )]
    ignore_region_live_at: bool,
}

macro_rules! attempt {
    ($($tokens:tt)*) => {
        (|| Ok({ $($tokens)* }))()
    };
}

pub fn main(opt: Opt) -> Result<(), Error> {
    let output_directory = opt
        .output_directory
        .as_ref()
        .map(|x| Path::new(x).to_owned());
    let graphviz_file = opt.graphviz_file.as_ref().map(|x| Path::new(x).to_owned());
    let liveness_graph_file = opt
        .liveness_graph_file
        .as_ref()
        .map(|x| Path::new(x).to_owned());
    for facts_dir in &opt.fact_dirs {
        let tables = &mut intern::InternerTables::new();

        let result: Result<(Duration, PoloniusFacts, PoloniusOutput), Error> = attempt! {
            let verbose = opt.verbose;
            let mut all_facts =
                tab_delim::load_tab_delimited_facts(tables, &Path::new(&facts_dir))?;
            if opt.ignore_region_live_at {
                all_facts.region_live_at = Vec::default();
            }
            let algorithm = opt.algorithm;
            let graphviz_output = graphviz_file.is_some() || liveness_graph_file.is_some();
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
                if opt.show_tuples {
                    dump::dump_output(&output, &output_directory, tables)
                        .expect("Failed to write output");
                }
                if let Some(ref graphviz_file) = graphviz_file {
                    dump::graphviz(&output, &all_facts, graphviz_file, tables)
                        .expect("Failed to write GraphViz");
                }
                if let Some(ref liveness_graph_file) = liveness_graph_file {
                    dump::liveness_graph(&output, &all_facts, liveness_graph_file, tables)
                        .expect("Failed to write liveness graph");
                }
            }

            Err(error) => {
                error!("`{}`: {}", facts_dir, error);
            }
        }
    }

    Ok(())
}

fn timed<T>(op: impl FnOnce() -> T) -> (Duration, T) {
    let start = Instant::now();
    let output = op();
    let duration = start.elapsed();
    (duration, output)
}
