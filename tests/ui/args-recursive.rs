use rusnix_ir as rusnix;

#[rusnix::args]
mod args {
    #[rusnix(root)]
    struct Root {
        nested: Nested,
    }

    struct Nested {
        next: Nested,
    }
}

fn main() {}
