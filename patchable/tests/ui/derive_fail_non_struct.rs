use patchable::Patchable;

#[derive(Patchable)]
enum NotAStruct {
    Value(i32),
}

fn main() {}
