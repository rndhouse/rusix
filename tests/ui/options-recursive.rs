use rusnix_ir as rusnix;

#[rusnix::options]
mod options {
    #[rusnix(root)]
    struct Root {
        nested: Nested,
    }

    struct Nested {
        next: Nested,
    }
}

fn main() {}
