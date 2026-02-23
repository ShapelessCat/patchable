use patchable::Patchable;

#[derive(Patchable)]
struct InvalidNestedPatchableType<T> {
    #[patchable]
    value: Option<T>,
}

fn main() {}
