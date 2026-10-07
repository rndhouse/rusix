use rusnix_ir as rusnix;

#[rusnix::args]
mod views {
    #[rusnix(root)]
    struct Inputs {
        #[rusnix(expression)]
        value: std::time::Duration,
    }
}

fn main() {}
