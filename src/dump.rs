use crate::facts::*;
use crate::intern::InternerTables;
use crate::intern::*;
use crate::output::Output;
use fxhash::FxHashMap;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::Hash;
use std::io::{self, Write};
use std::path::PathBuf;

crate fn dump_output(
    output: &Output<Region, Loan, Point>,
    output_dir: &Option<PathBuf>,
    intern: &InternerTables,
) -> io::Result<()> {
    dump_rows(
        &mut writer_for(output_dir, "borrow_live_at")?,
        intern,
        &output.borrow_live_at,
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
            &mut writer_for(output_dir, "potential_errors")?,
            intern,
            &output.potential_errors,
        )?;
        dump_rows(
            &mut writer_for(output_dir, "subset")?,
            intern,
            &output.subset,
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
    fn push_all(
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
    fn push_all(
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
    fn push_all(
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
    fn push_all(
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
    fn push_all(
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
    fn push_all(
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

fn preserve<'a>(s: &mut Vec<&'a str>, op: impl FnOnce(&mut Vec<&'a str>)) {
    let len = s.len();
    op(s);
    s.truncate(len);
}

crate trait Atom: Copy + From<usize> + Into<usize> {
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
