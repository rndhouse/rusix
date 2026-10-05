#[path = "../../examples/postgresql/model.rs"]
mod model;

fn main() {
    let _clauses = model::RoleClauses {
        nonexistent_privilege: Some(model::Clause::Enable),
        ..Default::default()
    };
}
