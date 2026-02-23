use patchable::Patchable;

#[derive(Patchable)]
struct InvalidPatchableSkipParameter<T> {
    #[patchable(skip, unknown)]
    value: T,
}

fn main() {}
