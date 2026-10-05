use rusnix_ir::{Config, Expr};

pub fn config(name: &str) -> Option<Config> {
    Some(match name {
        "good" => Config::new()
            .set("services.openssh.enable", true)
            .set("services.openssh.ports", vec![22])
            .set("logging.level", "verbose"),
        "bad-port" => Config::new().set(
            "services.openssh.ports",
            vec![Expr::int(70000).in_range(1, 65535, "SSH port must be in 1..=65535")],
        ),
        "nested" => Config::new().set(
            "services.openssh.ports",
            vec![Expr::int(22), Expr::int(44).divide(Expr::int(0))],
        ),
        "conflict" => Config::new()
            .set("service", true)
            .set("service.enable", true),
        "selective" => Config::new()
            .set("good", Expr::int(42))
            .set("bad", Expr::int(44).divide(Expr::int(0))),
        _ => return None,
    })
}
