use pest::Parser;
use pest_derive::Parser;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

#[derive(Parser)]
#[grammar = "mir.pest"]
struct MirParser;

pub fn parse(path: &Path) -> HashMap<String, Vec<String>> {
    let mut file = std::fs::File::open(&path).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();

    let mut pairs = MirParser::parse(Rule::func, &contents).unwrap_or_else(|e| panic!("{}", e));
    let func_pair = pairs.next().unwrap();

    let mut hm = HashMap::new();
    for pair in func_pair.into_inner() {
        match pair.as_rule() {
            Rule::block => {
                let mut iter = pair.into_inner();
                let bb = iter.next().unwrap();
                assert_eq!(bb.as_rule(), Rule::block_name);
                let bb_name = bb.as_str().replace("(cleanup):", "").replace(':', "");
                let bb_name = bb_name.trim();
                let mut v: Vec<String> = Vec::new();
                for instr in iter {
                    assert_eq!(instr.as_rule(), Rule::instruction);
                    v.push(instr.as_str().trim().to_owned());
                }
                let None = hm.insert(bb_name.to_owned(), v) else {
                    unreachable!()
                };
            }
            _ => {}
        }
    }
    hm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let path = Path::new(env!("HOME"))
            .join("rust")
            .join("tests")
            .join("mir-opt")
            .join("nll")
            .join("named_lifetimes_basic.use_x.nll.0.mir");
        let _ = parse(&path);

        let path = Path::new(env!("HOME"))
            .join("rust")
            .join("tests")
            .join("mir-opt")
            .join("storage_ranges.main.nll.0.mir");
        let _ = parse(&path);
    }
}
