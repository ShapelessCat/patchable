use patchable::Patchable;

#[derive(Patchable)]
struct BorrowedValue<'a> {
    value: &'a str,
}

fn main() {}
