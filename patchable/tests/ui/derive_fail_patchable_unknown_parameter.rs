use patchable::Patchable;

#[derive(Patchable)]
struct InvalidPatchableParameter<T> {
    #[patchable(unknown)]
    value: T,
}

fn main() {}
