use sargami::{Res, Sargami};

fn main() {
    let mut server = Sargami::new();
    server.get("/", |req| {
        println!("Req: {:?}", req);
        Res {
            status_code: 200,
            headers: vec![],
            body: "Hello, world get!".to_string(),
            status_text: "OK".to_string(),
        }
    });

    server.post("/", |req| {
        println!("Req: {:?}", req);
        Res {
            status_code: 200,
            headers: vec![],
            body: "Hello, world post!".to_string(),
            status_text: "OK".to_string(),
        }
    });

    server.put("/", |req| {
        println!("Req: {:?}", req);
        Res {
            status_code: 200,
            headers: vec![],
            body: "Hello, world put!".to_string(),
            status_text: "OK".to_string(),
        }
    });

    server.delete("/", |req| {
        println!("Req: {:?}", req);
        Res {
            status_code: 200,
            headers: vec![],
            body: "Hello, world delete!".to_string(),
            status_text: "OK".to_string(),
        }
    });

    let addr = "127.0.0.1:8080";
    server.listen(addr);

    println!("Hello, world!");
}
