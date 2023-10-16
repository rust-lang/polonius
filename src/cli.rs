use log::{error, Level, LevelFilter, Metadata, Record, SetLoggerError};
use pico_args as pico;
use polonius_engine::Algorithm;
use std::env;
use std::error;
use std::fmt;
use std::path::Path;
use std::process::exit;
use std::str::FromStr;
use std::time::{Duration, Instant};

use crate::dump;
use crate::dump::Output;
use crate::facts::AllFacts;
use crate::intern;
use crate::mir_parser;
use crate::tab_delim;

const PKG_NAME: &str = env!("CARGO_PKG_NAME");
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const PKG_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

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
    mir_file: Option<String>,
}

#[derive(Debug)]
pub struct Error(String);

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_str(&self.0)
    }
}

macro_rules! attempt {
    ($($tokens:tt)*) => {
        (|| Ok({ $($tokens)* }))()
    };
}

pub fn main(opt: Options) -> Result<(), Error> {
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

        let result: Result<(Duration, AllFacts, Output), Error> = attempt! {
            let verbose = opt.verbose;
            let all_facts = tab_delim::load_tab_delimited_facts(tables, &Path::new(&facts_dir))
                .map_err(|e| Error(e.to_string()))?;
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
                    let mir = opt
                        .mir_file
                        .as_ref()
                        .map(|x| mir_parser::parse(Path::new(&x)));
                    dump::graphviz(&output, &all_facts, graphviz_file, tables, &mir)
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

// Parses the provided CLI arguments into `Options`
pub fn options_from_args() -> Result<Options, Error> {
    let mut args = pico::Arguments::from_env();

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
        --graphviz-file <graphviz file>          Generate a graphviz file to visualize the computation
        --dump-liveness-graph <graphviz file>    Generate a graphviz file to visualize the liveness information
    -o, --output <output_directory>              Directory where to output resulting tuples

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
    let options = Options {
        algorithm: arg_from_str(&mut args, "-a")?.unwrap_or(Algorithm::Naive),
        show_tuples: args.contains("--show-tuples"),
        skip_timing: args.contains("--skip-timing"),
        verbose: args.contains(["-v", "--verbose"]),
        graphviz_file: arg_from_str(&mut args, "--graphviz-file")?,
        output_directory: arg_from_str(&mut args, "-o")?.or(arg_from_str(&mut args, "--output")?),
        liveness_graph_file: arg_from_str(&mut args, "--dump-liveness-graph")?,
        mir_file: arg_from_str(&mut args, "--mir-file")?,
        fact_dirs: args.free().map_err(readable_pico_error)?,
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

    // 4) setup logging at the default `Info` level when necessary
    if env::var("RUST_LOG").is_ok() {
        start_logging().expect("Initializing logger failed");
    }

    Ok(options)
}

// Read an argument from the CLI, parse it, but with a readable error message if it fails
pub fn arg_from_str<T>(args: &mut pico::Arguments, key: &'static str) -> Result<Option<T>, Error>
where
    T: FromStr,
    <T as FromStr>::Err: fmt::Display,
{
    args.value_from_str(key).map_err(|e| {
        Error(format!(
            "error parsing argument '{}': {}",
            key,
            readable_pico_error(e)
        ))
    })
}

// Make a pico_args error a bit more readable than just its `Debug` output
fn readable_pico_error(error: pico::Error) -> Error {
    use pico::Error;
    Error(match error {
        Error::ArgumentParsingFailed { cause } => format!("failed to parse ({})", cause),
        Error::Utf8ArgumentParsingFailed { value, cause } => {
            format!("'{}' isn't a valid value ({})", value, cause)
        }
        Error::OptionWithoutAValue(_) => "missing value".to_string(),
        Error::UnusedArgsLeft(left) => {
            format!("error, unrecognized arguments: {}", left.join(", "))
        }
        Error::NonUtf8Argument => "not a valid utf8 value".to_string(),
    })
}

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            eprintln!("{} {} - {}", record.level(), record.target(), record.args());
        }
    }

    fn flush(&self) {}
}

static LOGGER: Logger = Logger;

fn start_logging() -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER).map(|()| log::set_max_level(LevelFilter::Info))
}
