fn something() -> bool { true }

fn foo1() {
    let mut x = 22;
    let mut v = vec![];
    let p = &x;

    if something() {
        v.push(p);
        x += 1; //~ ERROR
    } else {
        x += 1;
    }

    drop(v);
}

fn foo2() {
    let mut x = 22;
    let mut v = vec![];
    let p = &x;

    if something() {
        v.push(p);
    } else {
        x += 1;
    }

    x += 1; //~ ERROR
    drop(v);
}

fn foo3() {
    let mut x = 22;
    let mut v = vec![];
    let p = &x;

    if something() {
        v.push(p);
    } else {
        x += 1;
    }

    drop(v);
}

fn main() { }
