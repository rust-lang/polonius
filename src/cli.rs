use crate::intern;
use crate::output::Output;
use crate::tab_delim;
use failure::Error;
use std::env;
use std::path::Path;

pub fn main() -> Result<(), Error> {
    do catch {
        for facts_dir in env::args().skip(1) {
            let tables = &mut intern::InternerTables::new();

            let result: Result<Output, Error> = do catch {
                let all_facts =
                    tab_delim::load_tab_delimited_facts(tables, Path::new(&facts_dir))?;
                Output::compute(all_facts, false)
            };

            match result {
                Ok(output) => {
                    output.dump(tables);
                }

                Err(error) => {
                    eprintln!("`{}`: {}", facts_dir, error);
                }
            }
        }
    }
}
