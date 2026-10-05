#[path = "../../examples/postgresql.rs"]
mod example;

fn main() {
    let _clauses = example::RoleClauses {
        nonexistent_privilege: Some(example::Clause::Enable),
        ..Default::default()
    };
}
