#[path = "../../examples/postgresql/model.rs"]
mod model;

fn main() {
    let _database = model::Database::Owned {
        name: "app".into(),
        owner_name: "other".into(),
        clauses: Default::default(),
    };
}
