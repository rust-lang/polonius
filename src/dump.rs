use crate::facts::*;
use crate::intern::InternerTables;
use crate::intern::*;
use log::info;
use petgraph::stable_graph::StableGraph;
use petgraph::visit::{Dfs, EdgeRef, IntoEdgeReferences, IntoNodeReferences, NodeIndexable};
use petgraph::{Incoming, Outgoing};
use polonius_engine::{Atom as PoloniusEngineAtom, Output as PoloniusEngineOutput};
use rustc_hash::FxHashMap;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::hash::Hash;
use std::io::{self, Write};
use std::path::PathBuf;

pub(crate) type Output = PoloniusEngineOutput<LocalFacts>;

pub(crate) fn dump_output(
    output: &Output,
    output_dir: &Option<PathBuf>,
    intern: &InternerTables,
) -> io::Result<()> {
    macro_rules! dump_output_fields {
        ( $($field:ident),+ ) => {
            $({
                let (name, mut write) = writer_for(output_dir, stringify!($field))?;
                dump_rows(
                    name,
                    &mut write,
                    intern,
                    &output.$field,
                )?;
            })+
        };
    }

    dump_output_fields![errors, move_errors];

    let (name, mut write) = writer_for(output_dir, "subset_errors")?;
    dump_rows(name, &mut write, intern, &output.subset_errors)?;

    if output.dump_enabled {
        dump_output_fields![
            origin_contains_loan_at,
            origin_contains_loan_anywhere,
            origin_live_on_entry,
            loan_invalidated_at,
            loan_live_at,
            subset_anywhere,
            known_contains,
            var_live_on_entry,
            var_drop_live_on_entry,
            path_maybe_initialized_on_exit,
            path_maybe_uninitialized_on_exit,
            var_maybe_partly_initialized_on_exit
        ];
    }
    return Ok(());

    fn writer_for(
        out_dir: &Option<PathBuf>,
        name: &str,
    ) -> io::Result<(Option<String>, Box<dyn Write>)> {
        // create a writer for the provided output.
        // If we have an output directory use that, otherwise just dump to stdout
        use std::fs;

        Ok(match out_dir {
            Some(dir) => {
                fs::create_dir_all(&dir)?;
                let mut of = dir.join(name);
                of.set_extension("facts");
                (None, Box::new(fs::File::create(of)?))
            }
            None => {
                let mut stdout = io::stdout();
                write!(&mut stdout, "# {}\n", name)?;
                (Some(name.to_string()), Box::new(stdout))
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
    name: Option<String>,
    stream: &mut dyn Write,
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

        if let Some(ref name) = name {
            write!(stream, "{} ", name)?;
        }
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

impl Atom for Origin {
    fn table(intern: &InternerTables) -> &Interner<Self> {
        &intern.origins
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

impl Atom for Path {
    fn table(intern: &InternerTables) -> &Interner<Self> {
        &intern.paths
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
        let (point, o) = point(f);
        by_point.entry(point).or_insert_with(Vec::new).push(o);
    }
    by_point
        .into_iter()
        .map(|(point, o)| {
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
            (point, s)
        })
        .collect()
}

fn build_inputs_by_point_for_visualization(
    all_facts: &AllFacts,
    intern: &InternerTables,
) -> Vec<HashMap<Point, String>> {
    vec![
        facts_by_point(
            all_facts.loan_issued_at.iter().cloned(),
            |(origin, loan, point)| (point, (origin, loan)),
            "loan_issued_at".to_string(),
            2,
            intern,
        ),
        facts_by_point(
            all_facts.loan_killed_at.iter().cloned(),
            |(loan, point)| (point, (loan,)),
            "loan_killed_at".to_string(),
            1,
            intern,
        ),
        facts_by_point(
            all_facts.subset_base.iter().cloned(),
            |(origin1, origin2, point)| (point, (origin1, origin2)),
            "subset_base".to_string(),
            2,
            intern,
        ),
        facts_by_point(
            all_facts.loan_invalidated_at.iter().cloned(),
            |(point, loan)| (point, (loan,)),
            "loan_invalidated_at".to_string(),
            0,
            intern,
        ),
        facts_by_point(
            all_facts.var_used_at.iter().cloned(),
            |(var, point)| (point, (var,)),
            "var_used_at".to_string(),
            1,
            intern,
        ),
        facts_by_point(
            all_facts.var_defined_at.iter().cloned(),
            |(var, point)| (point, (var,)),
            "var_defined_at".to_string(),
            1,
            intern,
        ),
        facts_by_point(
            all_facts.var_dropped_at.iter().cloned(),
            |(var, point)| (point, (var,)),
            "var_dropped_at".to_string(),
            1,
            intern,
        ),
        facts_by_point(
            all_facts.path_assigned_at_base.iter().cloned(),
            |(var, point)| (point, (var,)),
            "path_assigned_at_base".to_string(),
            1,
            intern,
        ),
        facts_by_point(
            all_facts.path_moved_at_base.iter().cloned(),
            |(var, point)| (point, (var,)),
            "path_moved_at_base".to_string(),
            1,
            intern,
        ),
        facts_by_point(
            all_facts.path_accessed_at_base.iter().cloned(),
            |(var, point)| (point, (var,)),
            "path_accessed_at_base".to_string(),
            1,
            intern,
        ),
    ]
}

fn build_outputs_by_point_for_visualization(
    output: &Output,
    intern: &InternerTables,
) -> Vec<HashMap<Point, String>> {
    vec![
        facts_by_point(
            output.loan_live_at.iter(),
            |(point, loans)| (*point, loans.clone()),
            "loan_live_at".to_string(),
            0,
            intern,
        ),
        facts_by_point(
            output.origin_contains_loan_at.iter(),
            |(point, origin_to_loans)| (*point, origin_to_loans.clone()),
            "origin_contains_loan_at".to_string(),
            0,
            intern,
        ),
        facts_by_point(
            output.loan_invalidated_at.iter(),
            |(point, loans)| (*point, loans.clone()),
            "loan_invalidated_at".to_string(),
            0,
            intern,
        ),
        facts_by_point(
            output.subset.iter(),
            |(point, origin_to_origins)| (*point, origin_to_origins.clone()),
            "subset".to_string(),
            0,
            intern,
        ),
        facts_by_point(
            output.var_live_on_entry.iter(),
            |(point, var)| (*point, var.clone()),
            "var_live_on_entry".to_string(),
            1,
            intern,
        ),
        facts_by_point(
            output.var_drop_live_on_entry.iter(),
            |(point, var)| (*point, var.clone()),
            "var_drop_live_on_entry".to_string(),
            1,
            intern,
        ),
        facts_by_point(
            output.origin_live_on_entry.iter(),
            |(point, origin)| (*point, origin.clone()),
            "origin_live_on_entry".to_string(),
            1,
            intern,
        ),
        facts_by_point(
            output.var_maybe_partly_initialized_on_exit.iter(),
            |(point, var)| (*point, var.clone()),
            "var_maybe_partly_initialized_on_exit".to_string(),
            1,
            intern,
        ),
        facts_by_point(
            output.path_maybe_initialized_on_exit.iter(),
            |(point, path)| (*point, path.clone()),
            "path_maybe_initialized_on_exit".to_string(),
            1,
            intern,
        ),
        facts_by_point(
            output.move_errors.iter(),
            |(point, path)| (*point, path.clone()),
            "move_errors".to_string(),
            1,
            intern,
        ),
    ]
}

pub(crate) fn graphviz(
    output: &Output,
    all_facts: &AllFacts,
    output_file: &PathBuf,
    intern: &InternerTables,
    mir: &Option<HashMap<String, Vec<String>>>,
) -> io::Result<()> {
    let mut file = File::create(output_file)?;
    let mut output_fragments: Vec<String> = Vec::new();
    let mut seen_nodes = BTreeSet::new();

    let inputs_by_point = build_inputs_by_point_for_visualization(all_facts, intern);
    let outputs_by_point = build_outputs_by_point_for_visualization(output, intern);

    output_fragments.push("digraph g {\n  graph [\n  rankdir = \"TD\"\n];\n".to_string());
    for (idx, &(point1, point2)) in all_facts.cfg_edge.iter().enumerate() {
        let graphviz_code = graphviz_for_edge(
            point1,
            point2,
            idx,
            &mut seen_nodes,
            &inputs_by_point,
            &outputs_by_point,
            intern,
            mir,
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
    point1: Point,
    point2: Point,
    edge_index: usize,
    seen_points: &mut BTreeSet<usize>,
    inputs_by_point: &[HashMap<Point, String>],
    outputs_by_point: &[HashMap<Point, String>],
    intern: &InternerTables,
    mir: &Option<HashMap<String, Vec<String>>>,
) -> Vec<String> {
    let mut ret = Vec::new();
    maybe_render_point(
        point1,
        seen_points,
        inputs_by_point,
        outputs_by_point,
        &mut ret,
        intern,
        mir,
    );
    maybe_render_point(
        point2,
        seen_points,
        inputs_by_point,
        outputs_by_point,
        &mut ret,
        intern,
        mir,
    );
    ret.push(format!(
        "\"node{0}\" -> \"node{1}\":f0 [\n  id = {2}\n];\n",
        point1.index(),
        point2.index(),
        edge_index
    ));
    ret
}

fn maybe_render_point(
    point: Point,
    seen_points: &mut BTreeSet<usize>,
    inputs_by_point: &[HashMap<Point, String>],
    outputs_by_point: &[HashMap<Point, String>],
    render_vec: &mut Vec<String>,
    intern: &InternerTables,
    mir: &Option<HashMap<String, Vec<String>>>,
) {
    if seen_points.contains(&point.index()) {
        return;
    }
    seen_points.insert(point.index());

    let input_tuples = inputs_by_point
        .iter()
        .filter_map(|inp| inp.get(&point).map(ToString::to_string))
        .collect::<Vec<_>>()
        .join(" | ");

    let output_tuples = outputs_by_point
        .iter()
        .filter_map(|outp| outp.get(&point).map(ToString::to_string))
        .collect::<Vec<_>>()
        .join(" | ");

    let point_str = escape_for_graphviz(Point::table(intern).untern(point));
    let (bb_name, offset) = extract(&point_str);
    let instr: String = mir
        .as_ref()
        .and_then(|hm| Some(format!("| {}", escape_for_graphviz(&hm[bb_name][offset]))))
        .unwrap_or_default();
    render_vec.push(format!("\"node{0}\" [\n  label = \"{{ <f0> {point_str} {instr} | INPUTS | {input_tuples} | OUTPUTS | {output_tuples} }}\"\n  shape = \"record\"\n];\n",
                     point.index(),
                    ));
}

fn extract(x: &str) -> (&str, usize) {
    let a = x.find('(').unwrap();
    let b = x.find('[').unwrap();
    let c = x.find(']').unwrap();
    let bb_name = &x[a + 1..b];
    let offset = &x[b + 1..c];
    (bb_name, offset.parse().unwrap())
}

fn escape_for_graphviz(s: &str) -> String {
    s.replace(r"\", r"\\")
        .replace("\"", "\\\"")
        .replace(r"(", r"\(")
        .replace(r")", r"\)")
        .replace("\n", r"\n")
        .to_string()
}

fn edge_live_vars(source: &Liveness, target: &Liveness) -> HashSet<Variable> {
    let edge_use_live_vars = source
        .use_live_vars
        .intersection(&target.use_live_vars)
        .cloned()
        .collect::<HashSet<_>>();

    let edge_drop_live_vars = source
        .drop_live_vars
        .intersection(&target.drop_live_vars)
        .cloned()
        .collect::<HashSet<_>>();

    edge_use_live_vars
        .union(&edge_drop_live_vars)
        .cloned()
        .collect()
}

#[derive(Debug)]
struct Liveness {
    use_live_vars: HashSet<Variable>,
    drop_live_vars: HashSet<Variable>,
    cfg_points: Vec<Point>,
    point_facts: Vec<(String, Variable, Point)>,
}

impl Liveness {
    fn extend(&mut self, other: Liveness) {
        self.use_live_vars.extend(other.use_live_vars);
        self.drop_live_vars.extend(other.drop_live_vars);
        self.cfg_points.extend(other.cfg_points);
        self.point_facts.extend(other.point_facts);
    }

    fn from_polonius_data(output: &Output, all_facts: &AllFacts, location: Point) -> Self {
        let mut point_facts = Vec::default();

        point_facts.extend(
            all_facts
                .var_defined_at
                .iter()
                .filter(|&(_var, point)| *point == location)
                .map(|&(var, point)| ("☠".to_string(), var, point)),
        );

        point_facts.extend(
            all_facts
                .var_dropped_at
                .iter()
                .filter(|&(_var, point)| *point == location)
                .map(|&(var, point)| ("💧".to_string(), var, point)),
        );

        point_facts.extend(
            all_facts
                .var_used_at
                .iter()
                .filter(|&(_var, point)| *point == location)
                .map(|&(var, point)| ("🔧".to_string(), var, point)),
        );

        Self {
            point_facts,
            use_live_vars: output
                .var_live_on_entry
                .get(&location)
                .map_or(HashSet::default(), |live| live.iter().cloned().collect()),

            drop_live_vars: output
                .var_drop_live_on_entry
                .get(&location)
                .map_or(HashSet::default(), |live| live.iter().cloned().collect()),
            cfg_points: vec![location],
        }
    }
}

fn render_cfg_label(node: &Liveness, intern: &InternerTables) -> String {
    let mut cfg_points = node.cfg_points.clone();
    cfg_points.sort();

    let mut fragments = vec![if cfg_points.len() <= 3 {
        node.cfg_points
            .iter()
            .map(|point| intern.points.untern(*point).replace("\"", ""))
            .collect::<Vec<String>>()
            .join(", ")
    } else {
        format!(
            "{}–{}",
            intern
                .points
                .untern(*cfg_points.first().unwrap())
                .replace("\"", ""),
            intern
                .points
                .untern(*cfg_points.last().unwrap())
                .replace("\"", "")
        )
    }];

    fragments[0].push_str("\\l");

    fragments.extend(node.point_facts.iter().map(|(label, var, point)| {
        format!(
            "{}({}, {}).",
            label,
            intern.variables.untern(*var).replace("\"", ""),
            intern.points.untern(*point).replace("\"", ""),
        )
    }));

    fragments.join("\\l")
}

pub(crate) fn liveness_graph(
    output: &Output,
    all_facts: &AllFacts,
    output_file: &PathBuf,
    intern: &InternerTables,
) -> io::Result<()> {
    info!("Generating liveness graph");
    let mut file = File::create(output_file)?;
    let mut output_fragments: Vec<String> = Vec::new();
    let mut cfg = StableGraph::<Liveness, ()>::new();
    let mut point_to_node = HashMap::new();

    for &(point1, point2) in all_facts.cfg_edge.iter() {
        let node1 = *point_to_node.entry(point1).or_insert_with(|| {
            cfg.add_node(Liveness::from_polonius_data(output, all_facts, point1))
        });

        let node2 = *point_to_node.entry(point2).or_insert_with(|| {
            cfg.add_node(Liveness::from_polonius_data(output, all_facts, point2))
        });

        cfg.add_edge(node1, node2, ());
    }

    info!("Reducing the liveness graph...");
    // CFG state reduction
    if let Some((first_point, _)) = all_facts.cfg_edge.first() {
        let first_node = *point_to_node.get(first_point).unwrap();
        let mut dfs = Dfs::new(&cfg, first_node);

        while let Some(node) = dfs.next(&cfg) {
            let out_degree = cfg.neighbors_directed(node, Outgoing).count();
            let in_degree = cfg.neighbors_directed(node, Incoming).count();

            // if we have an in-degree and out-degree of 1, we can safely merge
            // this node into the next one. Note that this check keeps the first
            // and last nodes!
            if out_degree == 1 && in_degree == 1 {
                let previous_node_idx = cfg
                    .neighbors_directed(node, Incoming)
                    .detach()
                    .next_node(&cfg)
                    .unwrap();

                let next_node_idx = cfg.neighbors(node).detach().next_node(&cfg).unwrap();

                let edge_live_set_before = edge_live_vars(&cfg[previous_node_idx], &cfg[node]);
                let edge_live_set_after = edge_live_vars(&cfg[node], &cfg[next_node_idx]);

                if edge_live_set_before == edge_live_set_after {
                    let node_data = cfg.remove_node(node).unwrap();
                    cfg[next_node_idx].extend(node_data);
                    cfg.add_edge(previous_node_idx, next_node_idx, ());
                }
            }
        }
    }

    output_fragments.push("digraph g {\n  graph [\n  rankdir = \"TD\"\n];\n".to_string()); // open digraph

    // output the nodes:
    output_fragments.push(
        cfg.node_references()
            .map(|(node_idx, node_data)| {
                format!(
                    "{} [shape=\"record\" label=\"{}\"]",
                    cfg.to_index(node_idx),
                    render_cfg_label(node_data, intern)
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    );

    output_fragments.push("\n\n".to_string());

    let mut edge_fragments = Vec::new();

    let colour_palette = vec![
        "#C6CDF7", "#899DA4", "#F98400", "#C7B19C", "#D67236", "#0F0D0E", "#FAEFD1", "#ECCBAE",
        "#E1AF00", "#74A089", "#DD8D29", "#85D4E3", "#1C1718", "#F8AFA8", "#CB2314", "#35274A",
        "#E1BD6D", "#FDDDA0", "#FD6467", "#ABDDDE", "#F2300F", "#D8B70A", "#EAD3BF", "#1E1E1E",
        "#273046", "#9C964A", "#046C9A", "#D9D0D3", "#FDD262", "#0B775E", "#4E2A1E", "#EABE94",
        "#D69C4E", "#E58601", "#F2AD00", "#CCC591", "#E1BD6D", "#35274A", "#FAD510", "#9B110E",
        "#81A88D", "#CEAB07", "#A42820", "#78B7C5", "#3F5151", "#B40F20", "#354823", "#F2300F",
        "#5B1A18", "#F3DF6C", "#DC863B", "#02401B", "#FAD77B", "#F1BB7B", "#7294D4", "#EABE94",
        "#39312F", "#550307", "#EBCC2A", "#972D15", "#A2A475", "#C27D38", "#24281A", "#0C1707",
        "#0B775E", "#D3DDDC", "#00A08A", "#F21A00", "#3B9AB2", "#E6A0C4", "#CDC08C", "#FF0000",
        "#9986A5", "#D5D5D3", "#79402E", "#D8A499", "#9A8822", "#46ACC8", "#CCBA72", "#E2D200",
        "#AA9486", "#F4B5BD", "#446455", "#8D8680", "#5BBCD6", "#798E87", "#5F5647", "#C93312",
        "#29211F", "#B6854D", "#e1f7d5", "#ffbdbd", "#c9c9ff", "#f1cbff",
    ];

    for edge in cfg.edge_references() {
        let edge_live_vars = edge_live_vars(&cfg[edge.source()], &cfg[edge.target()]);

        for &var in edge_live_vars.iter() {
            let liveness_status = vec![
                if cfg[edge.source()].use_live_vars.contains(&var) {
                    "U"
                } else {
                    ""
                },
                if cfg[edge.source()].drop_live_vars.contains(&var) {
                    "D"
                } else {
                    ""
                },
            ]
            .join("");

            edge_fragments.push(format!(
                "{} -> {} [label=\" {} {}\", color=\"{}\", penwidth = 2 arrowhead = none]",
                cfg.to_index(edge.target()),
                cfg.to_index(edge.source()),
                intern.variables.untern(var).replace("\"", ""),
                liveness_status,
                colour_palette[var.index() % colour_palette.len()],
            ));
        }

        edge_fragments.push(format!(
            "{} -> {} [penwidth = 2]",
            cfg.to_index(edge.source()),
            cfg.to_index(edge.target())
        ));
    }

    output_fragments.push(edge_fragments.join("\n"));

    output_fragments.push("\n}".to_string()); // close digraph
    let output_bytes = output_fragments.join("").bytes().collect::<Vec<_>>();
    file.write_all(&output_bytes)?;
    Ok(())
}
