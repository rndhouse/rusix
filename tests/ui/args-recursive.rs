use rusix_ir as rusix;

#[rusix::args]
mod args {
    #[rusix(root)]
    struct Root {
        nested: Nested,
    }

    struct Nested {
        next: Nested,
    }
}

fn main() {}
