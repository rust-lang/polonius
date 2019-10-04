//! A module with helpers to work with datalog and datafrog: containing
//! - a simple data model to analyze, and compute rule transformations.
//! - a primitive parser for _valid_ syntax creating instances of the data model.
//! - a simple datalog-to-datafrog generator which will generate a skeleton
//! datafrog computation of the datalog rules, including preparing data in
//! `Relations`, the computed `Variables`, the join/antijoin/map operations
//! translations of the rules, and setup and maintenance of the indices used during
//! the joins and their possibly intermediate steps.

use rustc_hash::{FxHashMap, FxHashSet};
use std::fmt::{self, Write};
use std::ops::Deref;

/// Whether a predicate is used only as input, or produces new tuples.
#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub enum PredicateKind {
    Extensional,
    Intensional,
}

/// An atom, or relational atom, is a building block used in rules, also known as subgoal,
/// describing a relation name and the name of its components.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Atom<'a> {
    pub predicate: String,
    pub args: Vec<&'a str>,
}

/// A richer type of relation/atom, which can be negated, and used as premises/hypotheses in rules.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Literal<'a> {
    pub atom: Atom<'a>,
    pub is_negated: bool,
    pub kind: PredicateKind,
}

/// A specific type of Horn clause relating the premises/hypotheses/antecedents/conditions in its body
/// to the conclusion/consequent in its head.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Rule<'a> {
    pub head: Atom<'a>,
    pub body: Vec<Literal<'a>>,
}

/// The representation of what a datalog rule does in datafrog terms
enum Operation<'a> {
    StaticMap(String),
    DynamicMap(String),
    Join(Vec<JoinStep<'a>>),
}

/// The representation of a join, with the data required to serialize it as Rust code
#[derive(Debug)]
struct JoinStep<'a> {
    src_a: String,
    src_b: String,

    is_antijoin: bool,

    key: Vec<&'a str>,
    args: Vec<&'a str>,

    remaining_args_a: Vec<&'a str>,
    remaining_args_b: Vec<&'a str>,

    dest_predicate: String,
    dest_key: Vec<&'a str>,
    dest_args: Vec<&'a str>,
}

/// Records the argument information of relation declarations
#[derive(Debug, Clone)]
struct ArgDecl {
    name: String,
    rust_type: String,
}

impl<'a> Atom<'a> {
    pub fn new(predicate: &'a str, args: Vec<&'a str>) -> Self {
        Atom {
            predicate: predicate.to_string(),
            args,
        }
    }
}

impl fmt::Display for Atom<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(", self.predicate)?;
        for (idx, arg) in self.args.iter().enumerate() {
            write!(f, "{}", arg)?;
            if idx < self.args.len() - 1 {
                write!(f, ", ")?;
            }
        }
        write!(f, ")")
    }
}

impl<'a> Literal<'a> {
    pub fn new(predicate: &'a str, args: Vec<&'a str>) -> Self {
        Self {
            atom: Atom::new(predicate, args),
            is_negated: false,
            kind: PredicateKind::Extensional,
        }
    }

    pub fn new_anti(predicate: &'a str, args: Vec<&'a str>) -> Self {
        Self {
            atom: Atom::new(predicate, args),
            is_negated: true,
            kind: PredicateKind::Extensional,
        }
    }
}

impl<'a> Deref for Literal<'a> {
    type Target = Atom<'a>;

    fn deref(&self) -> &Self::Target {
        &self.atom
    }
}

impl fmt::Display for Literal<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_negated {
            write!(f, "!")?;
        }
        write!(f, "{}", self.atom)
    }
}

impl fmt::Display for Rule<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} :- ", self.head)?;
        for (idx, h) in self.body.iter().enumerate() {
            write!(f, "{}", h)?;
            if idx < self.body.len() - 1 {
                write!(f, ", ")?;
            }
        }
        write!(f, ".")
    }
}

fn clean_program(text: String) -> String {
    text.lines()
        .map(|s| s.trim())
        .filter(|line| !line.starts_with("//"))
        .collect()
}

/// Primitive and inefficient parser for _valid_ datalog, with no error checking. Basically deserializing
/// a list of `Display` representations of a `Rule`, with a couple tweaks to make it easier
/// to write and use:
/// - ignores empty lines and the ones starting with `//`comments
/// - ignores whitespace between tokens
pub fn parse(text: &str) -> Vec<Rule<'_>> {
    let mut rules = Vec::new();

    for rule in text.split(".").map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let parts: Vec<_> = rule.split(":-").map(|s| s.trim()).collect();
        let head = parts[0];
        let body = parts[1].split("),");

        let head = {
            let idx = head.find("(").unwrap();
            let predicate = &head[..idx];
            let args = &head[idx..];

            let start = args.find("(").unwrap() + 1;
            let end = args.find(")").unwrap();
            let args: Vec<_> = args[start..end].split(", ").collect();

            Atom::new(predicate, args)
        };

        let body = {
            let string_literals = body.map(|s| s.trim());
            let mut body = Vec::new();

            for literal in string_literals {
                let idx = literal.find("(").unwrap();
                let mut predicate = &literal[..idx];
                let mut args = &literal[idx..];

                let is_negated = {
                    if predicate.starts_with("!") {
                        predicate = &predicate[1..];
                        true
                    } else {
                        false
                    }
                };

                let start = args.find("(").unwrap() + 1;
                if let Some(end) = args.find(")") {
                    args = &args[start..end];
                } else {
                    args = &args[start..];
                }

                let args: Vec<_> = args.split(", ").collect();

                let literal = if is_negated {
                    Literal::new_anti(predicate, args)
                } else {
                    Literal::new(predicate, args)
                };

                body.push(literal);
            }

            body
        };

        let rule = Rule { head, body };
        rules.push(rule);
    }

    rules
}

// Primitive parser of relation declarations:
// - one-per line
// - the syntax is similar to Soufflé's decls:
//   `.decl $relation($arg_a: TypeA, $arg_b: TypeB, ...)`
//
// The goal is to map from a relation name to its canonical argument names and ordering,
// which you can't have solely in rules (where variables/arguments can have arbitrary names).
// This is used when generating skeleton datafrog computations, to help naming the
// relation indices by the canonical variable names used in the index key.
fn parse_declarations(decls: &str) -> FxHashMap<String, Vec<ArgDecl>> {
    let mut declarations = FxHashMap::default();
    for line in decls.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
        let prefix = ".decl ".len();
        let line = &line[prefix..];

        let idx = line.find("(").unwrap();
        let predicate = &line[..idx];
        let args = &line[idx..];

        let start = args.find("(").unwrap() + 1;
        let end = args.find(")").unwrap();
        let args: Vec<_> = args[start..end]
            .split(",")
            .map(|arg| {
                let mut typed_arg_decl = arg.trim().split(":");
                let name = typed_arg_decl.next().unwrap().to_lowercase();
                let rust_type = typed_arg_decl.next().unwrap().trim().to_string();

                ArgDecl { name, rust_type }
            })
            .collect();

        declarations.insert(predicate.to_string(), args);
    }
    declarations
}

pub fn generate_skeleton_datafrog(decls: &str, text: &str, output: &mut String) {
    // Step 0: parse everything.
    let decls = parse_declarations(decls);
    let program = clean_program(text.to_string());
    let mut rules = parse(&program);

    // Step 1: analyze rules to separate extensional and intensional predicates.
    // These will end up being emitted as datafrog `Relation`s and `Variable`s, respectively.
    let mut intensional = FxHashSet::default();
    for rule in rules.iter() {
        intensional.insert(rule.head.predicate.clone());
    }

    let mut extensional = FxHashSet::default();
    for rule in rules.iter_mut() {
        for literal in rule.body.iter_mut() {
            if intensional.contains(&literal.predicate) {
                literal.kind = PredicateKind::Intensional;
            } else {
                extensional.insert(literal.predicate.clone());
            }
        }
    }

    // Step 2: visit rules and emit a datafrog "query plan".

    // Actually used predicates and indices
    let mut extensional_inputs = FxHashSet::default();
    let mut intensional_inputs = FxHashSet::default();

    let mut extensional_indices = FxHashMap::default();
    let mut intensional_indices = FxHashMap::default();

    // All relations used as keys need to be encoded as `((K, V), ())` tuples,
    // as the joins are done on the keys.
    let mut predicates_consumed_as_keys = FxHashSet::default();

    // The skeleton code:
    // - the inital data loading, before the loop, to fill the `Variable`s with
    //   data from the `Relation`s they join with in the rules.
    // - the dynamic computation data: the loop itself, executing the joins
    let mut generated_code_static_input: Vec<String> = Vec::new();
    let mut generated_code_dynamic_computation: Vec<String> = Vec::new();

    let mut operations = Vec::new();

    // Generate an `Operation` per rule, describing what the rule does, and
    // the data required to serialize it as rust code later. This is done in 2 steps
    // because we need to know which predicates are used as complete keys _before_
    // serializing them to code: the tuple produced by each rule would be different
    // depending on the join key of later rules.
    for (rule_idx, rule) in rules.iter().enumerate() {
        let body: Vec<_> = rule.body.iter().collect();

        let operation = match body.len() {
            0 => unreachable!(),

            1 => {
                // This a `map` operation.

                // Record used inputs to filter code generation later
                if !intensional.contains(&body[0].predicate) {
                    extensional_inputs.insert(body[0].predicate.clone());
                }

                let operation = {
                    // If this is mapping over an extensional predicate, we can emit
                    // this outside of the datalog computation loop, since the input is static.
                    if extensional.contains(&body[0].predicate) {
                        let operation = format!(
                            "{}.extend({}.iter().clone());\n",
                            rule.head.predicate, body[0].predicate
                        );
                        Operation::StaticMap(operation)
                    } else {
                        // otherwise, it's a map during computation

                        let args_a: Vec<_> = rule.head.args.clone();
                        let args_b: Vec<_> = body[0].args.clone();

                        let name_arg = |arg| {
                            if args_a.contains(arg) {
                                arg.to_string().to_lowercase()
                            } else {
                                format!("_{}", arg.to_string().to_lowercase())
                            }
                        };

                        let src_args = args_b.iter().map(name_arg).collect::<Vec<_>>().join(", ");
                        let mut dest_args =
                            args_a.iter().map(name_arg).collect::<Vec<_>>().join(", ");

                        if args_a.len() == 1 {
                            dest_args = format!("{}, ()", dest_args);
                        }

                        let operation = format!(
                            "{dest}.from_map(&{src}, |&({src_args})| ({dest_args}));\n",
                            dest = rule.head.predicate,
                            src = body[0].predicate,
                            dest_args = dest_args,
                            src_args = src_args,
                        );
                        Operation::DynamicMap(operation)
                    }
                };
                operation
            }

            _ => {
                // This is a `join` operation

                // TODO: check if there is only one intensional predicate and the rest are extensional
                // so that we can output a leapjoin instead of a regular join

                let mut steps: Vec<JoinStep> = Vec::new();

                for (literal_idx, literal) in body.iter().enumerate().skip(1) {
                    // We're joining 2 literals, but a step at a time (1 literal at a time),
                    // using the previous join output with the current step's literal.
                    // So the first `step_idx` of the join will start at `literal_idx` 1,
                    // joining `body[0]` and `body[1]`.
                    let step_idx = literal_idx - 1;

                    let is_first_step = step_idx == 0;
                    let is_last_step = literal_idx == body.len() - 1;

                    // TODO: datafrog has requirements that the joined Variable is the first
                    // argument, so if this is the first (or only) step of a join, the second literal
                    // should be a Variable (or emit an error), and swap the order here, or maybe also
                    // emit an error asking to swap in the source directly.

                    // When we're at the first step, there is no previous step result with which
                    // to join. But when we're at `literal_idx` of at least 2, we've joined 2
                    // literals already and continue to join that result, with the current step's literal.
                    let mut previous_step = if is_first_step {
                        None
                    } else {
                        Some(&mut steps[step_idx - 1])
                    };

                    // The destination where we produce our join's tuples can be:
                    // - a temporary relation, when we're at an intermediary of
                    //   the multiple-step join
                    // - the rule's conclusion when we're on the last step
                    let dest_predicate = if is_last_step {
                        rule.head.predicate.clone()
                    } else {
                        format!(
                            "{}_step_{}_{}",
                            rule.head.predicate,
                            rule_idx + 1,
                            step_idx + 1
                        )
                    };

                    // Record used inputs to filter code generation later
                    intensional.insert(dest_predicate.clone());

                    // The arguments to the source literals can either come from the
                    // first 2 literals in the body (at the firs step of the join),
                    // or from the previous step's result and current step's literal.
                    let args_a = if let Some(ref mut previous_step) = previous_step {
                        previous_step
                            .key
                            .iter()
                            .chain(previous_step.args.iter())
                            .map(|&v| v)
                            .collect()
                    } else {
                        body[0].args.clone()
                    };

                    let args_b = &literal.args;

                    // The join key is the shared variables between the 2 relations
                    let key: Vec<_> = args_b
                        .iter()
                        .filter(|&v| args_a.contains(v))
                        .map(|&v| v)
                        .collect();

                    // We now need to know which arguments were not used in the key: they will be the
                    // arguments that the datafrog closure producing the tuples of the join
                    // will _receive_.
                    let is_arg_used_later = |arg, skip| {
                        // if the argument is used in later steps, we need to retain it
                        for literal in body.iter().skip(skip) {
                            if literal.args.contains(arg) {
                                return true;
                            }
                        }

                        // similarly, if the variable is produced by the join process itself
                        if rule.head.args.contains(arg) {
                            return true;
                        }

                        // else, the argument is unused, and we can avoid producing it at this join step
                        false
                    };

                    let remaining_args_a: Vec<_> = args_a
                        .iter()
                        .filter(|v| !key.contains(v))
                        .filter(|v| is_arg_used_later(v, step_idx + 1))
                        .map(|&v| v)
                        .collect();
                    let remaining_args_b: Vec<_> = args_b
                        .iter()
                        .filter(|v| !key.contains(v))
                        .filter(|v| is_arg_used_later(v, step_idx + 1))
                        .map(|&v| v)
                        .collect();

                    // This step's arguments, which will be used by the next step when computing
                    // its join key.
                    let mut args = Vec::new();
                    for &arg in remaining_args_a.iter().chain(remaining_args_b.iter()) {
                        args.push(arg);
                    }

                    // Compute the source predicates:
                    // - if we're at the first step it'll be the first 2 literals
                    // - if we're at a later step, it'll be the previous step result, and the
                    //  current literal
                    //
                    // In both cases, predicates can be joined via some index, when only some of
                    // the arguments in the key are used. In this case, either source index can be
                    // used instead of the relation with full tuples.
                    //
                    // The "left" relation in the join could come from the previous step,
                    // in that case, there is no specific index to lookup.
                    //
                    let src_a = if let Some(ref mut previous_step) = previous_step {
                        previous_step.dest_predicate.clone()
                    } else {
                        if remaining_args_a.is_empty() {
                            body[0].predicate.clone()
                        } else {
                            generate_indexed_relation(
                                &decls,
                                &body[0],
                                &key,
                                &args_a,
                                &remaining_args_a,
                                &mut extensional,
                                &mut extensional_indices,
                                &mut intensional,
                                &mut intensional_inputs,
                                &mut intensional_indices,
                            )
                        }
                    };

                    let src_b = if remaining_args_b.is_empty() {
                        literal.predicate.clone()
                    } else {
                        generate_indexed_relation(
                            &decls,
                            &literal,
                            &key,
                            &args_b,
                            &remaining_args_b,
                            &mut extensional,
                            &mut extensional_indices,
                            &mut intensional,
                            &mut intensional_inputs,
                            &mut intensional_indices,
                        )
                    };

                    // The arguments that the datafrog closure will need to _produce_.
                    // Since these are only known in the next step, the next loop iteration
                    // will fill them. When we're at the last step, we produce what the rule
                    // asked us to produce in the first place.
                    let dest_args = if is_last_step {
                        rule.head.args.clone()
                    } else {
                        Vec::new()
                    };

                    // Now that we have computed what this join step requires from the previous step,
                    // we can back patch the previous one, to tell it what key-value tuples to produce.
                    if let Some(ref mut previous_step) = previous_step {
                        previous_step.dest_key = key.clone();
                        previous_step.dest_args = remaining_args_a.clone();
                    }

                    let is_antijoin = literal.is_negated;

                    if !is_antijoin {
                        if remaining_args_a.is_empty() {
                            predicates_consumed_as_keys.insert(src_a.clone());
                        }

                        if remaining_args_b.is_empty() {
                            predicates_consumed_as_keys.insert(src_b.clone());
                        }
                    }

                    let step = JoinStep {
                        src_a,
                        src_b,
                        is_antijoin,
                        key,
                        args,
                        remaining_args_a,
                        remaining_args_b,
                        dest_predicate,
                        dest_key: Vec::new(),
                        dest_args,
                    };

                    steps.push(step);
                }

                Operation::Join(steps)
            }
        };

        operations.push(operation);
    }

    // Serialize rule operations as string to generate the skeleton code
    for (rule_idx, (rule, operation)) in rules.iter().zip(operations.into_iter()).enumerate() {
        let rule_id = format!("R{:02}", rule_idx + 1);
        let rule_comment = format!("// {}: {}", rule_id, rule);

        generated_code_dynamic_computation.push(rule_comment.clone());

        match operation {
            Operation::StaticMap(text) => {
                generated_code_dynamic_computation.push(format!(
                    "// `{}` is a static input, already loaded into `{}`.",
                    rule.body[0].predicate, rule.head.predicate,
                ));
                generated_code_static_input.push(rule_comment);
                generated_code_static_input.push(text);
            }
            Operation::DynamicMap(text) => {
                generated_code_dynamic_computation.push(text);
            }
            Operation::Join(steps) => {
                for (step_idx, step) in steps.iter().enumerate() {
                    let is_last_step = step_idx == steps.len() - 1;

                    // Stringify the datafrog join closure arguments:
                    // - the key
                    // - the unused arguments from the first relation
                    // - the unused arguments from the second relation
                    let tupled_src_key =
                        join_args_as_tuple(&step.key, &step.dest_key, &step.dest_args);

                    let tupled_args_a = match step.remaining_args_a.len() {
                        0 => "_".to_string(),
                        _ => format!(
                            "&{}",
                            join_args_as_tuple(
                                &step.remaining_args_a,
                                &step.dest_key,
                                &step.dest_args
                            )
                        ),
                    };

                    let tupled_args_b = match step.remaining_args_b.len() {
                        0 => "_".to_string(),
                        _ => format!(
                            "&{}",
                            join_args_as_tuple(
                                &step.remaining_args_b,
                                &step.dest_key,
                                &step.dest_args
                            )
                        ),
                    };

                    // TODO: if this predicate's full row is used as join input elsewhere
                    // (it's not just an intensional predicate), then we need to encode it as a key/value tuple
                    // like `((key, value), ())`.
                    // Stringify the datafrog closure body: the value it will produce, and which can be
                    // a simple value, or a key-value tuple, depending on the join step, and the destination
                    // relation layout.
                    let mut produced_tuple = {
                        if is_last_step {
                            // we're on the last step, so we must produce what the rule's conclusion expects
                            step.dest_args.join(", ").to_lowercase()
                        } else {
                            // we're at an intermediary step of the multi-step join, so we must produce
                            // what the next step expects
                            let tupled_dest_key =
                                join_args_as_tuple(&step.dest_key, &step.dest_key, &step.dest_args);
                            let tupled_dest_args = join_args_as_tuple(
                                &step.dest_args,
                                &step.dest_key,
                                &step.dest_args,
                            );
                            format!("{}, {}", tupled_dest_key, tupled_dest_args)
                        }
                    };

                    // The encoding of these predicates consumed as keys requires to
                    // wrap the key-value tuple as a key in another tuple, and a unit value.
                    if predicates_consumed_as_keys.contains(&step.dest_predicate) {
                        produced_tuple = format!("({}), ()", produced_tuple);
                    }

                    let operation = if step.is_antijoin { "antijoin" } else { "join" };

                    // Adapt the closure signature to the specific join, we're doing. Antijoins
                    // consume all arguments, there will be no unused arguments for the join closure
                    // to receive.
                    let args = if step.is_antijoin {
                        tupled_args_a
                    } else {
                        format!(
                            "{args_a}, {args_b}",
                            args_a = tupled_args_a,
                            args_b = tupled_args_b,
                        )
                    };

                    // If either predicates is not intensional: it's either a declared extensional
                    // predicate, or one which was generated as an index of a declared relation,
                    // we'll record its use to only emit actually used `Relation`s.
                    // Technically, extensional predicates can only appear in the right element
                    // of a regular join; but we can reorder, and have to handle leapjoins, so let's
                    // check both right and left elements.
                    record_predicate_use(
                        &step.src_a,
                        &intensional,
                        &mut extensional_inputs,
                        &mut intensional_inputs,
                    );
                    record_predicate_use(
                        &step.src_b,
                        &intensional,
                        &mut extensional_inputs,
                        &mut intensional_inputs,
                    );

                    let operation = format!(
                        "{dest}.from_{operation}(&{src_a}, &{src_b}, |&{key}, {args}| ({tuple}));",
                        dest = step.dest_predicate,
                        operation = operation,
                        src_a = step.src_a,
                        src_b = step.src_b,
                        key = tupled_src_key,
                        args = args,
                        tuple = produced_tuple,
                    );
                    generated_code_dynamic_computation.push(operation);
                }
            }
        }

        // Add an empty line after every datalog rule conversion
        if rule_idx < rules.len() - 1 {
            generated_code_dynamic_computation.push("".to_string());
        }
    }

    // Infer the output of the computation: the difference between all the intensional
    // predicates and the ones used as inputs.
    let main_relation_candidates: Vec<_> = intensional
        .difference(&intensional_inputs)
        .cloned()
        .collect();

    println!(
        "{} extensional predicates/indices used (out of {}) and which can be a datafrog `Relation`:",
        extensional_inputs.len(),
        extensional.len(),
    );
    let mut extensional: Vec<_> = extensional_inputs.into_iter().collect();
    extensional.sort();
    for (idx, relation) in extensional.iter().enumerate() {
        println!("{:02}: `{}`", idx + 1, relation);
    }

    println!(
        "\n{} intensional predicates (including {} indices) requiring a datafrog `Variable`:",
        intensional.len(),
        intensional_indices.len(),
    );

    let mut intensional: Vec<_> = intensional.into_iter().collect();
    intensional.sort();
    for (idx, variable) in intensional.iter().enumerate() {
        let is_index = match intensional_indices.get(variable) {
            Some((original_literal, ..)) => format!(" (index on `{}`)", original_literal.predicate),
            None => "".to_string(),
        };

        println!("{:02}: `{}`{}", idx + 1, variable, is_index);
    }

    generate_skeleton_code(
        output,
        decls,
        extensional,
        extensional_indices,
        intensional,
        intensional_indices,
        predicates_consumed_as_keys,
        main_relation_candidates,
        generated_code_static_input,
        generated_code_dynamic_computation,
    )
    .expect("Skeleton code generation failed");
}

fn generate_skeleton_code(
    output: &mut String,
    decls: FxHashMap<String, Vec<ArgDecl>>,
    extensional_predicates: Vec<String>,
    extensional_indices: FxHashMap<String, (&String, String)>,
    intensional_predicates: Vec<String>,
    intensional_indices: FxHashMap<String, (&Literal<'_>, Vec<&str>, Vec<&str>)>,
    predicates_consumed_as_keys: FxHashSet<String>,
    main_relation_candidates: Vec<String>,
    generated_code_static_input: Vec<String>,
    generated_code_dynamic_computation: Vec<String>,
) -> fmt::Result {
    write!(output, "\n// Extensional predicates, and their indices\n\n")?;

    for relation in extensional_predicates.iter() {
        if let Some(arg_decls) = decls.get(relation) {
            // This is one the initial extensional predicates
            let arg_types: Vec<_> = arg_decls
                .iter()
                .map(|decl| decl.rust_type.as_ref())
                .collect();

            let arg_types = if predicates_consumed_as_keys.contains(relation) {
                format!("({}), ()", arg_types.join(", "))
            } else {
                arg_types.join(", ")
            };

            write!(
                output,
                "let {relation}: Relation<({arg_types})> = Vec::new().into();\n",
                relation = relation,
                arg_types = arg_types,
            )?;
        } else {
            // This is an index over an extensional predicate
            let (original_predicate, arg_types) = &extensional_indices[relation];

            let arg_types = if predicates_consumed_as_keys.contains(relation) {
                format!("({}), ()", arg_types)
            } else {
                arg_types.clone()
            };

            write!(
                output,
                "\n// Note: `{relation}` is an indexed version of the input facts `{original_predicate}`\n",
                relation = relation,
                original_predicate = original_predicate,
            )?;
            write!(
                output,
                "let {relation}: Relation<({arg_types})> = Vec::new().into();\n\n",
                relation = relation,
                arg_types = arg_types,
            )?;
        }
    }

    write!(output, "\n")?;

    // There can be only one 'main' intensional predicate
    if main_relation_candidates.len() == 1 {
        let main = &main_relation_candidates[0];
        write!(output, "// `{}` inferred as the output relation\n", main)?;
        write!(output, "let {} = {{\n", main)?;
    } else {
        write!(
            output,
            "// Note: couldn't infer output relation automatically\n"
        )?;
    }

    write!(output, "\nlet mut iteration = Iteration::new();")?;

    write!(output, "\n// Intensional predicates, and their indices\n\n")?;
    for variable in intensional_predicates.iter() {
        if let Some(arg_decls) = decls.get(variable) {
            // This is one of the initial intensional predicates
            let arg_types: Vec<_> = arg_decls
                .iter()
                .map(|decl| decl.rust_type.as_ref())
                .collect();

            let arg_types = if predicates_consumed_as_keys.contains(variable) {
                format!("({}), ()", arg_types.join(", "))
            } else {
                arg_types.join(", ")
            };

            write!(
                output,
                "let {variable} = iteration.variable::<({arg_types})>({variable:?});\n",
                variable = variable,
                arg_types = arg_types,
            )?;
        } else if let Some((original_literal, key, args)) = intensional_indices.get(variable) {
            let original_predicate = &original_literal.predicate;

            write!(output,
                "\n// Note: `{variable}` is an indexed version of the `{original_predicate}` relation\n",
                variable = variable,
                original_predicate = original_predicate,
            )?;

            let key_types: Vec<_> = key
                .iter()
                .map(|v| {
                    canonicalize_arg_type(&decls, original_predicate, &original_literal.args, v)
                        .to_string()
                })
                .collect();
            let args_types: Vec<_> = args
                .iter()
                .map(|v| {
                    canonicalize_arg_type(&decls, original_predicate, &original_literal.args, v)
                        .to_string()
                })
                .collect();

            let variable_type = join_types_as_tuple(key_types, args_types);
            let variable_type = if predicates_consumed_as_keys.contains(variable) {
                format!("({}), ()", variable_type)
            } else {
                variable_type
            };

            write!(
                output,
                "let {variable} = iteration.variable::<({variable_type})>({variable:?});\n",
                variable = variable,
                variable_type = variable_type,
            )?;
        } else {
            write!(
                output,
                "let {variable} = iteration.variable({variable:?});\n",
                variable = variable
            )?;
        }
    }

    // Initial data loading
    write!(output, "\n")?;
    for line in generated_code_static_input {
        write!(output, "{}\n", line)?;
    }

    write!(output, "while iteration.changed() {{\n")?;

    // Index maintenance
    write!(output, "\n    // Index maintenance\n")?;
    for (index_relation, (indexed_literal, key, args)) in intensional_indices.iter() {
        let indexed_relation = &indexed_literal.predicate;
        let arg_decls = &decls[indexed_relation];
        let arg_names: Vec<_> = arg_decls.iter().map(|decl| decl.name.as_ref()).collect();

        let tupled_args = join_args_as_tuple(&arg_names, &key, &args);

        let produced_key = join_args_as_tuple(&key, &key, &args);
        let produced_args = join_args_as_tuple(&args, &key, &args);

        write!(output,
            "    {index_relation}.from_map(&{indexed_relation}, |&{relation_args}| ({produced_key}, {produced_args}));\n",
            index_relation = index_relation,
            indexed_relation = indexed_relation,
            relation_args = tupled_args,
            produced_key = produced_key,
            produced_args = produced_args,
        )?;
    }

    // Finally, output the computation rules
    write!(output, "\n    // Rules\n\n")?;
    for line in generated_code_dynamic_computation {
        write!(output, "    {}\n", line)?;
    }

    write!(output, "}}\n")?;

    if main_relation_candidates.len() == 1 {
        write!(output, "\n{}.complete()\n", main_relation_candidates[0])?;
        write!(output, "}};\n")?;
    }

    Ok(())
}

fn generate_indexed_relation<'a>(
    decls: &FxHashMap<String, Vec<ArgDecl>>,
    literal: &'a Literal<'a>,
    key: &Vec<&'a str>,
    args: &Vec<&'a str>,
    remaining_args: &Vec<&'a str>,
    extensional_predicates: &mut FxHashSet<String>,
    extensional_indices: &mut FxHashMap<String, (&'a String, String)>,
    intensional_predicates: &mut FxHashSet<String>,
    intensional_inputs: &mut FxHashSet<String>,
    intensional_indices: &mut FxHashMap<String, (&Literal<'a>, Vec<&'a str>, Vec<&'a str>)>,
) -> String {
    let indexed_relation = generate_indexed_relation_name(&decls, &literal.predicate, &key, &args);

    // Index maintenance
    if extensional_predicates.contains(&literal.predicate) {
        let args_decls = &decls[&literal.predicate];
        record_extensional_index_use(
            &literal.predicate,
            &key,
            &remaining_args,
            args_decls,
            &indexed_relation,
            extensional_predicates,
            extensional_indices,
        );
    } else {
        record_intensional_index_use(
            &literal,
            &key,
            &remaining_args,
            &indexed_relation,
            intensional_predicates,
            intensional_inputs,
            intensional_indices,
        );
    }

    indexed_relation
}

fn record_predicate_use(
    predicate: &str,
    intensional_predicates: &FxHashSet<String>,
    extensional_inputs: &mut FxHashSet<String>,
    intensional_inputs: &mut FxHashSet<String>,
) {
    if !intensional_predicates.contains(predicate) {
        extensional_inputs.insert(predicate.to_string());
    } else {
        intensional_inputs.insert(predicate.to_string());
    }
}

fn record_extensional_index_use<'a>(
    predicate: &'a String,
    key: &Vec<&str>,
    args: &Vec<&str>,
    arg_decls: &Vec<ArgDecl>,
    indexed_relation: &str,
    extensional_predicates: &mut FxHashSet<String>,
    extensional_indices: &mut FxHashMap<String, (&'a String, String)>,
) {
    let key_types: Vec<_> = arg_decls
        .iter()
        .filter(|v| key.contains(&v.name.to_uppercase().as_ref()))
        .map(|decl| &decl.rust_type)
        .cloned()
        .collect();
    let arg_types: Vec<_> = arg_decls
        .iter()
        .filter(|v| args.contains(&v.name.to_uppercase().as_ref()))
        .map(|decl| &decl.rust_type)
        .cloned()
        .collect();

    extensional_predicates.insert(indexed_relation.to_string());
    extensional_indices.insert(
        indexed_relation.to_string(),
        (predicate, join_types_as_tuple(key_types, arg_types)),
    );
}

fn record_intensional_index_use<'a>(
    literal: &'a Literal<'a>,
    key: &Vec<&'a str>,
    args: &Vec<&'a str>,
    indexed_relation: &str,
    intensional_predicates: &mut FxHashSet<String>,
    intensional_inputs: &mut FxHashSet<String>,
    intensional_indices: &mut FxHashMap<String, (&Literal<'a>, Vec<&'a str>, Vec<&'a str>)>,
) {
    // When using an index, we're effectively using both `Variables`
    intensional_predicates.insert(indexed_relation.to_string());
    intensional_inputs.insert(literal.predicate.clone());

    intensional_indices.insert(
        indexed_relation.to_string(),
        (literal, key.clone(), args.clone()),
    );
}

fn find_arg_decl<'a>(
    global_decls: &'a FxHashMap<String, Vec<ArgDecl>>,
    predicate: &str,
    args: &Vec<&str>,
    variable: &str,
) -> &'a ArgDecl {
    let idx = args
        .iter()
        .position(|&arg| arg == variable)
        .expect("Couldn't find specified `variable` in the specified `args`");

    let predicate_arg_decls = &global_decls[predicate];
    let arg_decl = &predicate_arg_decls[idx];
    arg_decl
}

fn canonicalize_arg_name<'a>(
    global_decls: &'a FxHashMap<String, Vec<ArgDecl>>,
    predicate: &str,
    args: &Vec<&str>,
    variable: &str,
) -> &'a str {
    &find_arg_decl(global_decls, predicate, args, variable).name
}

fn canonicalize_arg_type<'a>(
    global_decls: &'a FxHashMap<String, Vec<ArgDecl>>,
    predicate: &str,
    args: &Vec<&str>,
    variable: &str,
) -> &'a str {
    &find_arg_decl(global_decls, predicate, args, variable).rust_type
}

fn generate_indexed_relation_name(
    decls: &FxHashMap<String, Vec<ArgDecl>>,
    predicate: &str,
    key: &Vec<&str>,
    args: &Vec<&str>,
) -> String {
    let mut index_args = String::new();
    for &v in key.iter() {
        let idx_key = canonicalize_arg_name(&decls, predicate, &args, v);
        index_args.push_str(&idx_key);
    }

    format!("{}_{}", predicate, index_args)
}

/// Generate tupled rust names for the datalog arguments, potentially prefixed
/// with _ to avoid generating a warning when it's not actually used
/// to produce the tuple, and potentially "untupled" if there's only one.
fn join_args_as_tuple(
    variables: &Vec<&str>,
    uses_key: &Vec<&str>,
    uses_args: &Vec<&str>,
) -> String {
    let name_arg = |arg| {
        if uses_key.contains(arg)
            || uses_key.contains(&arg.to_uppercase().as_ref())
            || uses_args.contains(arg)
            || uses_args.contains(&arg.to_uppercase().as_ref())
        {
            arg.to_string().to_lowercase()
        } else {
            format!("_{}", arg.to_string().to_lowercase())
        }
    };

    if variables.len() == 1 {
        name_arg(&variables[0])
    } else {
        format!(
            "({})",
            variables
                .iter()
                .map(name_arg)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn join_types_as_tuple(key_types: Vec<String>, args_types: Vec<String>) -> String {
    let join_as_tuple = |types: Vec<String>| {
        if types.len() == 1 {
            types[0].to_string()
        } else {
            format!("({})", types.into_iter().collect::<Vec<_>>().join(", "))
        }
    };

    let tupled_key_types = join_as_tuple(key_types);
    let tupled_args_types = join_as_tuple(args_types);
    format!("{}, {}", tupled_key_types, tupled_args_types)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_datalog() {
        let program = parse("p(x, y) :- e(x, y). p(x, z) :- e(x, y), p(y, z).");
        assert_eq!("p(x, y) :- e(x, y).", program[0].to_string());
        assert_eq!("p(x, z) :- e(x, y), p(y, z).", program[1].to_string());
    }

    #[test]
    fn parse_multiline_datalog() {
        let text = r#"
            subset(O1, O2, P)    :- outlives(O1, O2, P).
            subset(O1, O3, P)    :- subset(O1, O2, P), subset(O2, O3, P).
            subset(O1, O2, Q)    :- subset(O1, O2, P), cfg_edge(P, Q), region_live_at(O1, Q), region_live_at(O2, Q).
            requires(O, L, P)    :- borrow_region(O, L, P).
            requires(O2, L, P)   :- requires(O1, L, P), subset(O1, O2, P).
            requires(O, L, Q)    :- requires(O, L, P), !killed(L, P), cfg_edge(P, Q), region_live_at(O, Q).
            borrow_live_at(L, P) :- requires(O, L, P), region_live_at(O, P).
            errors(L, P)         :- invalidates(L, P), borrow_live_at(L, P)."#;

        let program = parse(text);
        let serialized = program
            .into_iter()
            .map(|rule| rule.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        let expected = r#"subset(O1, O2, P) :- outlives(O1, O2, P).
subset(O1, O3, P) :- subset(O1, O2, P), subset(O2, O3, P).
subset(O1, O2, Q) :- subset(O1, O2, P), cfg_edge(P, Q), region_live_at(O1, Q), region_live_at(O2, Q).
requires(O, L, P) :- borrow_region(O, L, P).
requires(O2, L, P) :- requires(O1, L, P), subset(O1, O2, P).
requires(O, L, Q) :- requires(O, L, P), !killed(L, P), cfg_edge(P, Q), region_live_at(O, Q).
borrow_live_at(L, P) :- requires(O, L, P), region_live_at(O, P).
errors(L, P) :- invalidates(L, P), borrow_live_at(L, P)."#;
        assert_eq!(expected, serialized);
    }

    #[test]
    fn parse_multiline_datalog_with_comments() {
        let text = r#"
            // `subset` rules
            subset(O1, O2, P) :- outlives(O1, O2, P).

            subset(O1, O3, P) :- subset(O1, O2, P),
                                   subset(O2, O3, P).
            subset(O1, O2, Q) :-
              subset(O1, O2, P),
              cfg_edge(P, Q),
              region_live_at(O1, Q),
              region_live_at(O2, Q).

            // `requires` rules
            requires(O, L, P) :- borrow_region(O, L, P).

            requires(O2, L, P) :-
              requires(O1, L, P),subset(O1, O2, P).

            requires(O, L, Q) :-
              requires(O, L, P),
                       !killed(L, P),    cfg_edge(P, Q),
    region_live_at(O, Q).

            // this one is commented out, nope(N, O, P, E) :- open(O, P, E, N).

            borrow_live_at(L, P) :-
              requires(O, L, P),
              region_live_at(O, P).

            errors(L, P) :-
              invalidates(L, P),
              borrow_live_at(L, P)."#;

        let program = clean_program(text.to_string());
        let rules = parse(&program);

        let serialized = rules
            .into_iter()
            .map(|rule| rule.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        let expected = r#"subset(O1, O2, P) :- outlives(O1, O2, P).
subset(O1, O3, P) :- subset(O1, O2, P), subset(O2, O3, P).
subset(O1, O2, Q) :- subset(O1, O2, P), cfg_edge(P, Q), region_live_at(O1, Q), region_live_at(O2, Q).
requires(O, L, P) :- borrow_region(O, L, P).
requires(O2, L, P) :- requires(O1, L, P), subset(O1, O2, P).
requires(O, L, Q) :- requires(O, L, P), !killed(L, P), cfg_edge(P, Q), region_live_at(O, Q).
borrow_live_at(L, P) :- requires(O, L, P), region_live_at(O, P).
errors(L, P) :- invalidates(L, P), borrow_live_at(L, P)."#;
        assert_eq!(expected, serialized);
    }

    #[test]
    fn generate_naive_rules() {
        let decls = r#"
            .decl borrow_region(O: Origin, L: Loan, P: Point)
            .decl cfg_edge(P: Point, Q: Point)
            .decl killed(L: Loan, P: Point)
            .decl outlives(O1: Origin, O2: Origin, P: Point)
            .decl region_live_at(O: Origin, P: Point)
            .decl subset(O1: Origin, O2: Origin, P: Point)
            .decl requires(O: Origin, L: Loan, P: Point)
            .decl borrow_live_at(L: Loan, P: Point)
            .decl invalidates(L: Loan, P: Point)
            .decl errors(L: Loan, P: Point)
        "#;

        let rules = r#"
            subset(O1, O2, P)    :- outlives(O1, O2, P).
            subset(O1, O3, P)    :- subset(O1, O2, P), subset(O2, O3, P).
            subset(O1, O2, Q)    :- subset(O1, O2, P), cfg_edge(P, Q), region_live_at(O1, Q), region_live_at(O2, Q).
            requires(O, L, P)    :- borrow_region(O, L, P).
            requires(O2, L, P)   :- requires(O1, L, P), subset(O1, O2, P).
            requires(O, L, Q)    :- requires(O, L, P), !killed(L, P), cfg_edge(P, Q), region_live_at(O, Q).
            borrow_live_at(L, P) :- requires(O, L, P), region_live_at(O, P).
            errors(L, P)         :- borrow_live_at(L, P), invalidates(L, P).
        "#;

        let mut output = String::new();
        generate_skeleton_datafrog(decls, rules, &mut output);

        let expected = r#"
// Extensional predicates, and their indices

let borrow_region: Relation<(Origin, Loan, Point)> = Vec::new().into();

// Note: `cfg_edge_p` is an indexed version of the input facts `cfg_edge`
let cfg_edge_p: Relation<(Point, Point)> = Vec::new().into();

let invalidates: Relation<((Loan, Point), ())> = Vec::new().into();
let killed: Relation<(Loan, Point)> = Vec::new().into();
let outlives: Relation<(Origin, Origin, Point)> = Vec::new().into();
let region_live_at: Relation<((Origin, Point), ())> = Vec::new().into();

// `errors` inferred as the output relation
let errors = {

let mut iteration = Iteration::new();
// Intensional predicates, and their indices

let borrow_live_at = iteration.variable::<((Loan, Point), ())>("borrow_live_at");
let errors = iteration.variable::<(Loan, Point)>("errors");
let requires = iteration.variable::<(Origin, Loan, Point)>("requires");

// Note: `requires_lp` is an indexed version of the `requires` relation
let requires_lp = iteration.variable::<((Loan, Point), Origin)>("requires_lp");

// Note: `requires_op` is an indexed version of the `requires` relation
let requires_op = iteration.variable::<((Origin, Point), Loan)>("requires_op");
let requires_step_6_1 = iteration.variable("requires_step_6_1");
let requires_step_6_2 = iteration.variable("requires_step_6_2");
let subset = iteration.variable::<(Origin, Origin, Point)>("subset");

// Note: `subset_o1p` is an indexed version of the `subset` relation
let subset_o1p = iteration.variable::<((Origin, Point), Origin)>("subset_o1p");

// Note: `subset_o2p` is an indexed version of the `subset` relation
let subset_o2p = iteration.variable::<((Origin, Point), Origin)>("subset_o2p");

// Note: `subset_p` is an indexed version of the `subset` relation
let subset_p = iteration.variable::<(Point, (Origin, Origin))>("subset_p");
let subset_step_3_1 = iteration.variable("subset_step_3_1");
let subset_step_3_2 = iteration.variable("subset_step_3_2");

// R01: subset(O1, O2, P) :- outlives(O1, O2, P).
subset.extend(outlives.iter().clone());

// R04: requires(O, L, P) :- borrow_region(O, L, P).
requires.extend(borrow_region.iter().clone());

while iteration.changed() {

    // Index maintenance
    requires_op.from_map(&requires, |&(o, l, p)| ((o, p), l));
    requires_lp.from_map(&requires, |&(o, l, p)| ((l, p), o));
    subset_o2p.from_map(&subset, |&(o1, o2, p)| ((o2, p), o1));
    subset_o1p.from_map(&subset, |&(o1, o2, p)| ((o1, p), o2));
    subset_p.from_map(&subset, |&(o1, o2, p)| (p, (o1, o2)));

    // Rules

    // R01: subset(O1, O2, P) :- outlives(O1, O2, P).
    // `outlives` is a static input, already loaded into `subset`.
    
    // R02: subset(O1, O3, P) :- subset(O1, O2, P), subset(O2, O3, P).
    subset.from_join(&subset_o2p, &subset_o1p, |&(_o2, p), &o1, &o3| (o1, o3, p));
    
    // R03: subset(O1, O2, Q) :- subset(O1, O2, P), cfg_edge(P, Q), region_live_at(O1, Q), region_live_at(O2, Q).
    subset_step_3_1.from_join(&subset_p, &cfg_edge_p, |&_p, &(o1, o2), &q| ((o1, q), o2));
    subset_step_3_2.from_join(&subset_step_3_1, &region_live_at, |&(o1, q), &o2, _| ((o2, q), o1));
    subset.from_join(&subset_step_3_2, &region_live_at, |&(o2, q), &o1, _| (o1, o2, q));
    
    // R04: requires(O, L, P) :- borrow_region(O, L, P).
    // `borrow_region` is a static input, already loaded into `requires`.
    
    // R05: requires(O2, L, P) :- requires(O1, L, P), subset(O1, O2, P).
    requires.from_join(&requires_op, &subset_o1p, |&(_o1, p), &l, &o2| (o2, l, p));
    
    // R06: requires(O, L, Q) :- requires(O, L, P), !killed(L, P), cfg_edge(P, Q), region_live_at(O, Q).
    requires_step_6_1.from_antijoin(&requires_lp, &killed, |&(l, p), &o| (p, (l, o)));
    requires_step_6_2.from_join(&requires_step_6_1, &cfg_edge_p, |&_p, &(l, o), &q| ((o, q), l));
    requires.from_join(&requires_step_6_2, &region_live_at, |&(o, q), &l, _| (o, l, q));
    
    // R07: borrow_live_at(L, P) :- requires(O, L, P), region_live_at(O, P).
    borrow_live_at.from_join(&requires_op, &region_live_at, |&(_o, p), &l, _| ((l, p), ()));
    
    // R08: errors(L, P) :- borrow_live_at(L, P), invalidates(L, P).
    errors.from_join(&borrow_live_at, &invalidates, |&(l, p), _, _| (l, p));
}

errors.complete()
};
"#;
        println!("{}", output);
        assert_eq!(expected, output);
    }
}
