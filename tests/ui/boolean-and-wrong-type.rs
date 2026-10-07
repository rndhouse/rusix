use rusix_ir::Expr;

fn main() {
    let _ = Expr::boolean(true).and(Expr::int(42));
}
