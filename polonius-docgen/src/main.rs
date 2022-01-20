use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::{error, fmt};

#[derive(Debug)]
pub struct Error(String);

impl error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_str(&self.0)
    }
}

fn main() -> Result<(), Error> {
    let infile = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!(
            "Error: Missing datalog file

Usage:\n\tpolonius-docgen <datalog_file>"
        );
        std::process::exit(1);
    });
    datalog_to_markdown(&infile)
}

fn datalog_to_markdown(filename: &impl AsRef<Path>) -> Result<(), Error> {
    let filename = filename.as_ref();
    let file = File::open(filename).map_err(|_| Error(format!("Cannot open file {filename:?}")))?;

    let mut comments = Vec::new();
    let mut code = Vec::new();
    for (i, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|_| Error(format!("Read file {filename:?} failed")))?;

        match line.trim() {
            // An empty line ends the section
            "" => {
                if !comments.is_empty() || !code.is_empty() {
                    write_section(std::mem::take(&mut comments), std::mem::take(&mut code));
                }
            }
            // Preprocessor directives
            line if line.starts_with('#') => (),
            // A comment
            line if line.starts_with("//") => {
                assert!(code.is_empty());

                let line = line.strip_prefix("//").unwrap().trim_start();

                comments.push(line.to_string());
            }
            // A line of code
            line => {
                // Add a section header for datalog declarations
                let mut words = line.split_whitespace();
                if [".type", ".decl"].contains(&words.next().unwrap()) {
                    let title = words
                        .next()
                        .unwrap_or("")
                        .split(|ch: char| !ch.is_ascii_alphabetic() && ch != '_')
                        .next()
                        .ok_or_else(|| {
                            Error(format!(
                                "Invalid Type or Relation at line {}: {line:?}",
                                i + 1
                            ))
                        })?;
                    let h = "####";
                    comments.insert(0, format!("{h} `{title}`"));
                    comments.insert(1, String::new());
                }
                code.push(line.to_string());
            }
        }
    }
    write_section(comments, code);
    Ok(())
}

fn write_section(comments: Vec<String>, code: Vec<String>) {
    for line in &comments {
        println!("{line}");
    }
    if matches!(comments.last(), Some(line) if !line.is_empty()) {
        println!();
    }

    if !code.is_empty() {
        println!("```prolog");
        for line in &code {
            println!("{line}");
        }
        println!("```\n");
    }
}
