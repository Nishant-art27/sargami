# sargami

A zero-dependency HTTP/1.1 micro-framework for Rust with an Express-style API.

It is built on `std` alone and started as a learning project. It is small
enough to read in one sitting and is not meant for production traffic.

## Example

```rust
use sargami::{Res, Sargami};

fn main() -> std::io::Result<()> {
    let mut app = Sargami::new();

    app.get("/", |_| Res::ok().with_body("hello"))
        .get("/greet", |req| {
            let name = req.get_query("name").unwrap_or("stranger");
            Res::ok().with_body(format!("hello, {name}"))
        })
        .post("/echo", |req| Res::ok().with_body(req.body.unwrap_or_default()));

    app.listen("127.0.0.1:8080")
}
```

A runnable version lives in [`examples/example`](examples/example):

```sh
cd examples/example
cargo run
curl 'http://127.0.0.1:8080/greet?name=sargam'
```

## What it does

- Routes `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD` and `OPTIONS` by exact path
- Parses query strings, including percent-decoding and `+` as space
- Reads `Content-Length` bodies up to 1 MiB and answers `413` above that
- Accepts closures that capture state as handlers, as long as they are `Send + Sync`
- Serves each connection on its own thread
- Answers malformed requests with `400` instead of crashing

## What it does not do yet

- Path parameters or wildcards (`/users/:id`)
- Keep-alive: every response closes the connection
- Chunked transfer encoding
- TLS

## Development

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs the same three commands on every push.

## License

MIT. See [LICENSE](LICENSE).
