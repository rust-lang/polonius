use crate::facts::*;
use crate::intern::*;
use fxhash::FxHashMap;
use std::hash::Hash;

crate trait OutputDump {
    fn push_all(
        &'a self,
        intern: &'a InternerTables,
        prefix: &mut Vec<&'a str>,
        output: &mut Vec<Vec<&'a str>>,
    );
}

crate fn dump_rows(title: &str, intern: &InternerTables, value: &impl OutputDump) {
    println!("# {}", title);
    println!();

    let mut rows = Vec::new();
    OutputDump::push_all(value, intern, &mut vec![], &mut rows);
    let col_width: usize = rows.iter()
        .map(|cols| cols.iter().map(|s| s.len()).max().unwrap_or(0))
        .max()
        .unwrap_or(0);
    for row in &rows {
        let mut string = String::new();

        for col in row {
            string.push_str(col);

            let padding = col_width - col.len();
            for _ in 0..padding {
                string.push(' ');
            }
        }

        println!("{}", string);
    }
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
