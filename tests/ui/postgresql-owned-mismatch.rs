#[path = "../../examples/postgresql.rs"]
mod example;

fn main() {
    let _database = example::Database::Owned {
        name: "app".into(),
        owner_name: "other".into(),
        clauses: Default::default(),
    };
}
