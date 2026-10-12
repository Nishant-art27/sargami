use sargami::{Res, Sargami};

fn main() -> std::io::Result<()> {
    let mut server = Sargami::new();

    server
        .get("/", |_| Res::ok().with_body("Hello, world!"))
        .get("/greet", |req| {
            let name = req.get_query("name").unwrap_or("stranger");
            Res::ok().with_body(format!("Hello, {name}!"))
        })
        .post("/echo", |req| Res::ok().with_body(req.body.unwrap_or_default()))
        .put("/", |_| Res::ok().with_body("Hello, world put!"))
        .delete("/", |_| Res::ok().with_body("Hello, world delete!"));

    server.listen("127.0.0.1:8080")
}
