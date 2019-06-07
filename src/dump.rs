use crate::facts::*;
use crate::intern::InternerTables;
use crate::intern::*;
use polonius_engine::{Atom as PoloniusEngineAtom, Output};
use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::hash::Hash;
use std::io::{self, Write};
use std::path::PathBuf;

pub(crate) fn dump_output(
    output: &Output<Region, Loan, Point, Variable>,
    output_dir: &Option<PathBuf>,
    intern: &InternerTables,
) -> io::Result<()> {
    dump_rows(
        &mut writer_for(output_dir, "errors")?,
        intern,
        &output.errors,
    )?;

    if output.dump_enabled {
        dump_rows(
            &mut writer_for(output_dir, "restricts")?,
            intern,
            &output.restricts,
        )?;
        dump_rows(
            &mut writer_for(output_dir, "restricts_anywhere")?,
            intern,
            &output.restricts_anywhere,
        )?;
        dump_rows(
            &mut writer_for(output_dir, "region_live_at")?,
            intern,
            &output.region_live_at,
        )?;
        dump_rows(
            &mut writer_for(output_dir, "invalidates")?,
            intern,
            &output.invalidates,
        )?;
        dump_rows(
            &mut writer_for(output_dir, "borrow_live_at")?,
            intern,
            &output.borrow_live_at,
        )?;
        dump_rows(
            &mut writer_for(output_dir, "subset_anywhere")?,
            intern,
            &output.subset_anywhere,
        )?;
    }
    return Ok(());

    fn writer_for(out_dir: &Option<PathBuf>, name: &str) -> io::Result<Box<Write>> {
        // create a writer for the provided output.
        // If we have an output directory use that, otherwise just dump to stdout
        use std::fs;

        Ok(match out_dir {
            Some(dir) => {
                fs::create_dir_all(&dir)?;
                let mut of = dir.join(name);
                of.set_extension("facts");
                Box::new(fs::File::create(of)?)
            }
            None => {
                let mut stdout = io::stdout();
                write!(&mut stdout, "# {}\n\n", name)?;
                Box::new(stdout)
            }
        })
    }
}

trait OutputDump {
    fn push_all<'a>(
        &'a self,
        intern: &'a InternerTables,
        prefix: &mut Vec<&'a str>,
        output: &mut Vec<Vec<&'a str>>,
    );
}

fn dump_rows(
    stream: &mut Write,
    intern: &InternerTables,
    value: &impl OutputDump,
) -> io::Result<()> {
    let mut rows = Vec::new();
    OutputDump::push_all(value, intern, &mut vec![], &mut rows);
    let col_width: usize = rows
        .iter()
        .map(|cols| cols.iter().map(|s| s.len()).max().unwrap_or(0))
        .max()
        .unwrap_or(0);
    for row in &rows {
        let mut string = String::new();

        let (last, not_last) = row.split_last().unwrap();
        for col in not_last {
            string.push_str(col);

            let padding = col_width - col.len();
            for _ in 0..=padding {
                string.push(' ');
            }
        }
        string.push_str(last);

        writeln!(stream, "{}", string)?;
    }

    Ok(())
}

impl<K, V> OutputDump for FxHashMap<K, V>
where
    K: Atom + Eq + Hash + Ord,
    V: OutputDump,
{
    fn push_all<'a>(
        &'a self,
        intern: &'a InternerTables,
        prefix: &mut Vec<&'a str>,
        output: &mut Vec<Vec<&'a str>>,
    ) {
        let table = K::table(intern);
        let mut keys: Vec<_> = self.keys().collect();
        keys.sort();
        for key in keys {
            preserve(prefix, |prefix| {
                prefix.push(table.untern(*key));

                let value = &self[key];
                value.push_all(intern, prefix, output);
            });
        }
    }
}

impl<K, V> OutputDump for BTreeMap<K, V>
where
    K: Atom + Eq + Hash + Ord,
    V: OutputDump,
{
    fn push_all<'a>(
        &'a self,
        intern: &'a InternerTables,
        prefix: &mut Vec<&'a str>,
        output: &mut Vec<Vec<&'a str>>,
    ) {
        let table = K::table(intern);
        let mut keys: Vec<_> = self.keys().collect();
        keys.sort();
        for key in keys {
            preserve(prefix, |prefix| {
                prefix.push(table.untern(*key));

                let value = &self[key];
                value.push_all(intern, prefix, output);
            });
        }
    }
}

impl<K> OutputDump for BTreeSet<K>
where
    K: OutputDump,
{
    fn push_all<'a>(
        &'a self,
        intern: &'a InternerTables,
        prefix: &mut Vec<&'a str>,
        output: &mut Vec<Vec<&'a str>>,
    ) {
        for key in self {
            key.push_all(intern, prefix, output);
        }
    }
}

impl<V> OutputDump for Vec<V>
where
    V: OutputDump,
{
    fn push_all<'a>(
        &'a self,
        intern: &'a InternerTables,
        prefix: &mut Vec<&'a str>,
        output: &mut Vec<Vec<&'a str>>,
    ) {
        for value in self {
            value.push_all(intern, prefix, output);
        }
    }
}

impl<T: Atom> OutputDump for T {
    fn push_all<'a>(
        &'a self,
        intern: &'a InternerTables,
        prefix: &mut Vec<&'a str>,
        output: &mut Vec<Vec<&'a str>>,
    ) {
        let table = T::table(intern);
        let text = table.untern(*self);
        preserve(prefix, |prefix| {
            prefix.push(text);
            output.push(prefix.clone());
        });
    }
}

impl<T1: Atom> OutputDump for (T1,) {
    fn push_all<'a>(
        &'a self,
        intern: &'a InternerTables,
        prefix: &mut Vec<&'a str>,
        output: &mut Vec<Vec<&'a str>>,
    ) {
        let (ref a1,) = self;
        let t1_table = T1::table(intern);
        let a1_text = t1_table.untern(*a1);
        preserve(prefix, |prefix| {
            prefix.push(a1_text);
            output.push(prefix.clone());
        });
    }
}

impl<T1: Atom, T2: Atom> OutputDump for (T1, T2) {
    fn push_all<'a>(
        &'a self,
        intern: &'a InternerTables,
        prefix: &mut Vec<&'a str>,
        output: &mut Vec<Vec<&'a str>>,
    ) {
        let (ref a1, ref a2) = self;
        let t1_table = T1::table(intern);
        let t2_table = T2::table(intern);
        let a1_text = t1_table.untern(*a1);
        let a2_text = t2_table.untern(*a2);
        preserve(prefix, |prefix| {
            prefix.push(a1_text);
            prefix.push(a2_text);
            output.push(prefix.clone());
        });
    }
}

fn preserve<'a>(s: &mut Vec<&'a str>, op: impl FnOnce(&mut Vec<&'a str>)) {
    let len = s.len();
    op(s);
    s.truncate(len);
}

pub(crate) trait Atom: Copy + From<usize> + Into<usize> {
    fn table(intern: &InternerTables) -> &Interner<Self>;
}

impl Atom for Region {
    fn table(intern: &InternerTables) -> &Interner<Self> {
        &intern.regions
    }
}

impl Atom for Point {
    fn table(intern: &InternerTables) -> &Interner<Self> {
        &intern.points
    }
}

impl Atom for Loan {
    fn table(intern: &InternerTables) -> &Interner<Self> {
        &intern.loans
    }
}

impl Atom for Variable {
    fn table(intern: &InternerTables) -> &Interner<Self> {
        &intern.variables
    }
}

fn facts_by_point<F: Clone, Out: OutputDump>(
    facts: impl Iterator<Item = F>,
    point: impl Fn(F) -> (Point, Out),
    name: String,
    point_pos: usize,
    intern: &InternerTables,
) -> HashMap<Point, String> {
    let mut by_point: HashMap<Point, Vec<Out>> = HashMap::new();
    for f in facts {
        let (p, o) = point(f);
        by_point.entry(p).or_insert_with(Vec::new).push(o);
    }
    by_point
        .into_iter()
        .map(|(p, o)| {
            let mut rows: Vec<Vec<&str>> = Vec::new();
            OutputDump::push_all(&o, intern, &mut vec![], &mut rows);
            let s = rows
                .into_iter()
                .map(|mut vals| {
                    vals.insert(point_pos, "_");
                    escape_for_graphviz(
                        format!(
                            "{}({})",
                            name,
                            vals.into_iter()
                                .map(std::string::ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                        .as_str(),
                    )
                })
                .collect::<Vec<_>>()
                .join("\\l")
                + "\\l";
            // in graphviz, \l is a \n that left-aligns
            (p, s)
        })
        .collect()
}

fn build_inputs_by_point_for_visualization(
    all_facts: &AllFacts,
    intern: &InternerTables,
) -> Vec<HashMap<Point, String>> {
    vec![
        facts_by_point(
            all_facts.borrow_region.iter().cloned(),
            |(a, b, p)| (p, (a, b)),
            "borrow_region".to_string(),
            2,
            intern,
        ),
        facts_by_point(
            all_facts.killed.iter().cloned(),
            |(l, p)| (p, (l,)),
            "killed".to_string(),
            1,
            intern,
        ),
        facts_by_point(
            all_facts.outlives.iter().cloned(),
            |(r1, r2, p)| (p, (r1, r2)),
            "outlives".to_string(),
            2,
            intern,
        ),
        facts_by_point(
            all_facts.region_live_at.iter().cloned(),
            |(r, p)| (p, (r,)),
            "region_live_at".to_string(),
            1,
            intern,
        ),
        facts_by_point(
            all_facts.invalidates.iter().cloned(),
            |(p, l)| (p, (l,)),
            "invalidates".to_string(),
            0,
            intern,
        ),
        facts_by_point(
            all_facts.var_used.iter().cloned(),
            |(v, p)| (p, (v,)),
            "var_used".to_string(),
            1,
            intern,
        ),
        facts_by_point(
            all_facts.var_defined.iter().cloned(),
            |(v, p)| (p, (v,)),
            "var_defined".to_string(),
            1,
            intern,
        ),
        facts_by_point(
            all_facts.var_drop_used.iter().cloned(),
            |(v, p)| (p, (v,)),
            "var_drop_used".to_string(),
            1,
            intern,
        ),
    ]
}

fn build_outputs_by_point_for_visualization(
    output: &Output<Region, Loan, Point, Variable>,
    intern: &InternerTables,
) -> Vec<HashMap<Point, String>> {
    vec![
        facts_by_point(
            output.borrow_live_at.iter(),
            |(pt, loans)| (*pt, loans.clone()),
            "borrow_live_at".to_string(),
            0,
            intern,
        ),
        facts_by_point(
            output.restricts.iter(),
            |(pt, region_to_loans)| (*pt, region_to_loans.clone()),
            "restricts".to_string(),
            0,
            intern,
        ),
        facts_by_point(
            output.invalidates.iter(),
            |(pt, loans)| (*pt, loans.clone()),
            "invalidates".to_string(),
            0,
            intern,
        ),
        facts_by_point(
            output.subset.iter(),
            |(pt, region_to_regions)| (*pt, region_to_regions.clone()),
            "subset".to_string(),
            0,
            intern,
        ),
        facts_by_point(
            output.var_live_at.iter(),
            |(p, v)| (*p, v.clone()),
            "var_live_at".to_string(),
            1,
            intern,
        ),
        facts_by_point(
            output.var_drop_live_at.iter(),
            |(p, v)| (*p, v.clone()),
            "var_drop_live_at".to_string(),
            1,
            intern,
        ),
        facts_by_point(
            output.region_live_at.iter(),
            |(pt, region)| (*pt, region.clone()),
            "region_live_at".to_string(),
            0,
            intern,
        ),
    ]
}

pub(crate) fn graphviz(
    output: &Output<Region, Loan, Point, Variable>,
    all_facts: &AllFacts,
    output_file: &PathBuf,
    intern: &InternerTables,
) -> io::Result<()> {
    let mut file = File::create(output_file)?;
    let mut output_fragments: Vec<String> = Vec::new();
    let mut seen_nodes = BTreeSet::new();

    let inputs_by_point = build_inputs_by_point_for_visualization(all_facts, intern);
    let outputs_by_point = build_outputs_by_point_for_visualization(output, intern);

    output_fragments.push("digraph g {\n  graph [\n  rankdir = \"TD\"\n];\n".to_string());
    for (idx, &(p1, p2)) in all_facts.cfg_edge.iter().enumerate() {
        let graphviz_code = graphviz_for_edge(
            p1,
            p2,
            idx,
            &mut seen_nodes,
            &inputs_by_point,
            &outputs_by_point,
            intern,
        )
        .into_iter();
        output_fragments.extend(graphviz_code);
    }
    output_fragments.push("}".to_string()); // close digraph
    let output_bytes = output_fragments.join("").bytes().collect::<Vec<_>>();
    file.write_all(&output_bytes)?;
    Ok(())
}

fn graphviz_for_edge(
    p1: Point,
    p2: Point,
    edge_index: usize,
    seen_points: &mut BTreeSet<usize>,
    inputs_by_point: &[HashMap<Point, String>],
    outputs_by_point: &[HashMap<Point, String>],
    intern: &InternerTables,
) -> Vec<String> {
    let mut ret = Vec::new();
    maybe_render_point(
        p1,
        seen_points,
        inputs_by_point,
        outputs_by_point,
        &mut ret,
        intern,
    );
    maybe_render_point(
        p2,
        seen_points,
        inputs_by_point,
        outputs_by_point,
        &mut ret,
        intern,
    );
    ret.push(format!(
        "\"node{0}\" -> \"node{1}\":f0 [\n  id = {2}\n];\n",
        p1.index(),
        p2.index(),
        edge_index
    ));
    ret
}

fn maybe_render_point(
    pt: Point,
    seen_points: &mut BTreeSet<usize>,
    inputs_by_point: &[HashMap<Point, String>],
    outputs_by_point: &[HashMap<Point, String>],
    render_vec: &mut Vec<String>,
    intern: &InternerTables,
) {
    if seen_points.contains(&pt.index()) {
        return;
    }
    seen_points.insert(pt.index());

    let input_tuples = inputs_by_point
        .iter()
        .filter_map(|inp| inp.get(&pt).map(std::string::ToString::to_string))
        .collect::<Vec<_>>()
        .join(" | ");

    let output_tuples = outputs_by_point
        .iter()
        .filter_map(|outp| outp.get(&pt).map(std::string::ToString::to_string))
        .collect::<Vec<_>>()
        .join(" | ");

    render_vec.push(format!("\"node{0}\" [\n  label = \"{{ <f0> {1} | INPUTS | {2} | OUTPUTS | {3} }}\"\n  shape = \"record\"\n];\n",
                     pt.index(),
                     escape_for_graphviz(Point::table(intern).untern(pt)),
                     &input_tuples,
                     &output_tuples));
}

fn escape_for_graphviz(s: &str) -> String {
    s.replace(r"\", r"\\")
        .replace("\"", "\\\"")
        .replace(r"(", r"\(")
        .replace(r")", r"\)")
        .replace("\n", r"\n")
        .to_string()
}
