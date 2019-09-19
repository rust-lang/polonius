use crate::dump;
use crate::dump::Output;
use crate::facts::AllFacts;
use crate::intern;
use crate::tab_delim;
use log::error;
use pico_args::Arguments;
use polonius_engine::Algorithm;
use std::error::Error;
use std::path::Path;
use std::process::exit;
use std::time::{Duration, Instant};

const PKG_NAME: &'static str = env!("CARGO_PKG_NAME");
const PKG_VERSION: &'static str = env!("CARGO_PKG_VERSION");
const PKG_DESCRIPTION: &'static str = env!("CARGO_PKG_DESCRIPTION");

#[derive(Debug)]
pub struct Options {
    algorithm: Algorithm,
    show_tuples: bool,
    skip_timing: bool,
    verbose: bool,
    graphviz_file: Option<String>,
    output_directory: Option<String>,
    fact_dirs: Vec<String>,
    liveness_graph_file: Option<String>,
}

macro_rules! attempt {
    ($($tokens:tt)*) => {
        (|| Ok({ $($tokens)* }))()
    };
}

pub fn main(opt: Options) -> Result<(), Box<dyn Error>> {
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

        let result: Result<(Duration, AllFacts, Output), Box<dyn Error>> = attempt! {
            let verbose = opt.verbose;
            let all_facts =
                tab_delim::load_tab_delimited_facts(tables, &Path::new(&facts_dir))?;
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

impl Options {
    pub fn from_args() -> Result<Options, Box<dyn Error>> {
        let mut args = Arguments::from_env();

        // 1) print optional information before exiting: help, version
        let show_help = args.contains(["-h", "--help"]);
        if show_help {
            let variants: Vec<_> = Algorithm::variants()
                .iter()
                .map(|s| s.to_string())
                .collect();

            println!(
                r#"{name} {version}
{description}

USAGE:
    polonius [FLAGS] [OPTIONS] <fact_dirs>...

FLAGS:
    -h, --help           Prints help information
        --show-tuples    Show output tuples on stdout
        --skip-timing    Do not display timing results
    -V, --version        Prints version information
    -v, --verbose        Show intermediate output tuples and not just errors

OPTIONS:
    -a <algorithm> [default: Naive]
        [possible values: {variants}]
        --graphviz_file <graphviz_file>                Generate a graphviz file to visualize the computation
        --dump-liveness-graph <liveness_graph_file>    Generate a graphviz file to visualize the liveness information
    -o, --output <output_directory>                    Directory where to output resulting tuples

ARGS:
    <fact_dirs>..."#,
                name = PKG_NAME,
                version = PKG_VERSION,
                description = PKG_DESCRIPTION,
                variants = variants.join(", ")
            );
            exit(0);
        }

        // print version if needed
        if args.contains("-V") {
            println!("{} {}", PKG_NAME, PKG_VERSION);
            exit(0);
        }

        // 2) parse args
        // TODO: the error printed when `value_from_str` is called is terrible.
        // The new unreleased version of pico_args (current: 0.1) will allow to get the error enum, and print what we need.
        // Finish this when it's released !
        let options = Options {
            algorithm: args.value_from_str("-a")?.unwrap_or(Algorithm::Naive),
            show_tuples: args.contains("--show-tuples"),
            skip_timing: args.contains("--skip-timing"),
            verbose: args.contains(["-v", "--verbose"]),
            graphviz_file: args.value_from_str("--graphviz_file")?,
            output_directory: args
                .value_from_str("-o")?
                .or(args.value_from_str("--output")?),
            liveness_graph_file: args.value_from_str("--dump-liveness-graph")?,
            fact_dirs: args.free()?,
        };

        // 3) validate args: a fact directory is required
        if options.fact_dirs.is_empty() {
            println!(
                r#"error: The following required arguments were not provided:
    <fact_dirs>...

USAGE:
    polonius <fact_dirs>... -a <algorithm>

For more information try --help"#
            );
            exit(1);
        }

        Ok(options)
    }
}
