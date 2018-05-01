extern crate borrow_check;
extern crate failure;

pub fn main() -> Result<(), failure::Error> {
    borrow_check::cli::main()
}
