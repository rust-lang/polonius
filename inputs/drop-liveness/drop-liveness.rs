#[derive(Debug)]
struct Foo<'a> {
    data: &'a u32,
}

impl<'a> Drop for Foo<'a> {
    fn drop(&mut self) {
        /* we assume this could do something like `*self.data;` */
        println!("dropping, we had {:?}", self.data);
    }
}

fn main() {
    let x = 13;
    let y = Foo { data: &x };
    println!("y = {:?}", y);
    println!("x = {:?}", x);
}
