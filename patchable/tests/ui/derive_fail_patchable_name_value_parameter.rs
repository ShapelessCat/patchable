use patchable::Patchable;

#[derive(Patchable)]
struct InvalidPatchableNameValueParameter<T> {
    #[patchable = "skip"]
    value: T,
}

fn main() {}
