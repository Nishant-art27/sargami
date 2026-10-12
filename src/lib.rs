//! A zero-dependency HTTP/1.1 micro-framework with an Express-style API.
//!
//! ```no_run
//! use sargami::{Res, Sargami};
//!
//! let mut app = Sargami::new();
//! app.get("/", |_| Res::ok().with_body("hello"))
//!     .get("/greet", |req| {
//!         let name = req.get_query("name").unwrap_or("stranger");
//!         Res::ok().with_body(format!("hello, {name}"))
//!     });
//! app.listen("127.0.0.1:8080").unwrap();
//! ```

use std::{
    collections::HashMap,
    io::{self, BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    thread,
};

mod method;
mod req;
mod res;

pub use method::{Method, UnknownMethod};
pub use req::{BodyError, MAX_BODY_SIZE, ParseError, Req};
pub use res::Res;

/// A request handler: anything callable as `Fn(Req) -> Res` that can be shared
/// between threads, including closures that capture state.
pub type Handler = Box<dyn Fn(Req) -> Res + Send + Sync>;

/// The router and server. Register handlers with [`Sargami::get`] and friends,
/// then call [`Sargami::listen`].
#[derive(Default)]
pub struct Sargami {
    routes: HashMap<(Method, String), Handler>,
}

impl Sargami {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `handler` for requests with `method` on exactly `path`.
    /// Returns `&mut Self` so registrations can be chained.
    pub fn route<F>(&mut self, method: Method, path: &str, handler: F) -> &mut Self
    where
        F: Fn(Req) -> Res + Send + Sync + 'static,
    {
        self.routes
            .insert((method, path.to_string()), Box::new(handler));
        self
    }

    pub fn get<F>(&mut self, path: &str, handler: F) -> &mut Self
    where
        F: Fn(Req) -> Res + Send + Sync + 'static,
    {
        self.route(Method::Get, path, handler)
    }

    pub fn post<F>(&mut self, path: &str, handler: F) -> &mut Self
    where
        F: Fn(Req) -> Res + Send + Sync + 'static,
    {
        self.route(Method::Post, path, handler)
    }

    pub fn put<F>(&mut self, path: &str, handler: F) -> &mut Self
    where
        F: Fn(Req) -> Res + Send + Sync + 'static,
    {
        self.route(Method::Put, path, handler)
    }

    pub fn patch<F>(&mut self, path: &str, handler: F) -> &mut Self
    where
        F: Fn(Req) -> Res + Send + Sync + 'static,
    {
        self.route(Method::Patch, path, handler)
    }

    pub fn delete<F>(&mut self, path: &str, handler: F) -> &mut Self
    where
        F: Fn(Req) -> Res + Send + Sync + 'static,
    {
        self.route(Method::Delete, path, handler)
    }

    fn handle_request(&self, method: Method, req: Req) -> Res {
        match self.routes.get(&(method, req.path.clone())) {
            Some(handler) => handler(req),
            None => Res::not_found(),
        }
    }

    fn handle_connection(&self, stream: TcpStream) {
        let mut reader = BufReader::new(&stream);

        // Read the request line and headers, up to and including the blank line.
        let mut head = String::new();
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) if line == "\r\n" || line == "\n" => break,
                Ok(_) => head.push_str(&line),
                Err(e) => {
                    eprintln!("Error reading request: {e}");
                    return;
                }
            }
        }

        // A client that connected and hung up without sending anything is not
        // an error: browsers open speculative connections like this all the time.
        if head.is_empty() {
            return;
        }

        let mut req = match Req::parse(&head) {
            Ok(req) => req,
            Err(e) => {
                eprintln!("Bad request: {e}");
                send(&stream, &Res::bad_request());
                return;
            }
        };

        // The body follows the blank line. It has to come from the same buffered
        // reader, which may already be holding some of its bytes.
        if let Err(e) = req.read_body(&mut reader) {
            eprintln!("Bad request: {e}");
            let res = match e {
                BodyError::TooLarge(_) => Res::payload_too_large(),
                _ => Res::bad_request(),
            };
            send(&stream, &res);
            return;
        }

        let method = match req.method.parse::<Method>() {
            Ok(method) => method,
            Err(e) => {
                eprintln!("Bad request: {e}");
                send(&stream, &Res::bad_request());
                return;
            }
        };

        send(&stream, &self.handle_request(method, req));
    }

    /// Bind to `addr` and serve requests forever, one thread per connection.
    pub fn listen(&self, addr: &str) -> io::Result<()> {
        let listener = TcpListener::bind(addr)?;
        println!("Listening on {addr}");
        self.serve(listener)
    }

    /// Serve requests on an already-bound listener forever, one thread per
    /// connection. Bind to port 0 and read back `local_addr()` first when the
    /// port must be known before serving, as the tests do.
    pub fn serve(&self, listener: TcpListener) -> io::Result<()> {
        thread::scope(|scope| {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        scope.spawn(move || self.handle_connection(stream));
                    }
                    Err(e) => eprintln!("Error accepting connection: {e}"),
                }
            }
        });
        Ok(())
    }
}

/// Write a response to the client. A client that has already hung up must not
/// take the server down with it, so write errors are logged, not unwrapped.
fn send(mut stream: &TcpStream, res: &Res) {
    let result = stream
        .write_all(res.to_string().as_bytes())
        .and_then(|_| stream.flush());
    if let Err(e) = result {
        eprintln!("Error sending response: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    /// Start `app` on a free port in a background thread and return its address.
    /// The listener is bound before the thread starts, so there is nothing to
    /// wait for: connections made right away queue until the server accepts them.
    fn start(app: Sargami) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || app.serve(listener));
        addr
    }

    /// Send a raw request and return the whole response.
    fn exchange(addr: SocketAddr, raw: &str) -> String {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream.write_all(raw.as_bytes()).unwrap();
        stream.flush().unwrap();

        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    #[test]
    fn routes_a_get_request() {
        let mut app = Sargami::new();
        app.get("/sargam", |_| Res::ok().with_body("hello world"));
        let addr = start(app);

        let response = exchange(addr, "GET /sargam HTTP/1.1\r\nHost: localhost\r\n\r\n");
        assert!(
            response.starts_with("HTTP/1.1 200 OK\r\n"),
            "got: {response}"
        );
        assert!(response.ends_with("\r\n\r\nhello world"), "got: {response}");
    }

    #[test]
    fn unknown_route_is_a_404() {
        let addr = start(Sargami::new());

        let response = exchange(addr, "GET /nope HTTP/1.1\r\nHost: localhost\r\n\r\n");
        assert!(
            response.starts_with("HTTP/1.1 404 Not Found\r\n"),
            "got: {response}"
        );
    }

    #[test]
    fn survives_bad_clients() {
        let mut app = Sargami::new();
        app.get("/ok", |_| Res::ok().with_body("still alive"));
        let addr = start(app);

        // A client that connects and hangs up without sending anything.
        drop(TcpStream::connect(addr).unwrap());

        // A request line with no HTTP version.
        let response = exchange(addr, "GET /ok\r\n\r\n");
        assert!(response.starts_with("HTTP/1.1 400"), "got: {response}");

        // A header line with no colon.
        let response = exchange(addr, "GET /ok HTTP/1.1\r\nnot a header\r\n\r\n");
        assert!(response.starts_with("HTTP/1.1 400"), "got: {response}");

        // A method the server does not know.
        let response = exchange(addr, "BREW /ok HTTP/1.1\r\n\r\n");
        assert!(response.starts_with("HTTP/1.1 400"), "got: {response}");

        // The server must still be alive to answer a well-formed request.
        let response = exchange(addr, "GET /ok HTTP/1.1\r\nHost: localhost\r\n\r\n");
        assert!(response.starts_with("HTTP/1.1 200"), "got: {response}");
    }

    #[test]
    fn post_body_reaches_the_handler() {
        let mut app = Sargami::new();
        app.post("/echo", |req| {
            Res::ok().with_body(req.body.unwrap_or_default())
        });
        let addr = start(app);

        let request = concat!(
            "POST /echo HTTP/1.1\r\n",
            "Host: localhost\r\n",
            "Content-Length: 11\r\n",
            "\r\n",
            "hello world"
        );
        let response = exchange(addr, request);
        assert!(response.starts_with("HTTP/1.1 200"), "got: {response}");
        assert!(response.ends_with("\r\n\r\nhello world"), "got: {response}");
    }

    #[test]
    fn query_string_does_not_break_routing() {
        let mut app = Sargami::new();
        app.get("/users", |req| {
            Res::ok().with_body(req.get_query("id").unwrap_or("none").to_string())
        });
        let addr = start(app);

        let response = exchange(addr, "GET /users?id=42 HTTP/1.1\r\nHost: localhost\r\n\r\n");
        assert!(response.starts_with("HTTP/1.1 200"), "got: {response}");
        assert!(response.ends_with("\r\n\r\n42"), "got: {response}");
    }

    #[test]
    fn handlers_can_capture_state() {
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);

        let mut app = Sargami::new();
        app.get("/hit", move |_| {
            let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
            Res::ok().with_body(n.to_string())
        });
        let addr = start(app);

        exchange(addr, "GET /hit HTTP/1.1\r\n\r\n");
        exchange(addr, "GET /hit HTTP/1.1\r\n\r\n");
        let response = exchange(addr, "GET /hit HTTP/1.1\r\n\r\n");

        assert!(response.ends_with("\r\n\r\n3"), "got: {response}");
        assert_eq!(hits.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn route_registration_chains() {
        let mut app = Sargami::new();
        app.get("/a", |_| Res::ok().with_body("a"))
            .post("/b", |_| Res::ok().with_body("b"))
            .route(Method::Options, "/c", |_| Res::ok());
        let addr = start(app);

        let response = exchange(addr, "POST /b HTTP/1.1\r\n\r\n");
        assert!(response.ends_with("\r\n\r\nb"), "got: {response}");

        let response = exchange(addr, "OPTIONS /c HTTP/1.1\r\n\r\n");
        assert!(response.starts_with("HTTP/1.1 200"), "got: {response}");
    }

    #[test]
    fn handles_connections_concurrently() {
        let mut app = Sargami::new();
        app.get("/slow", |_| {
            thread::sleep(Duration::from_millis(300));
            Res::ok()
        });
        let addr = start(app);

        let started = Instant::now();
        let clients: Vec<_> = (0..2)
            .map(|_| thread::spawn(move || exchange(addr, "GET /slow HTTP/1.1\r\n\r\n")))
            .collect();
        for client in clients {
            let response = client.join().unwrap();
            assert!(response.starts_with("HTTP/1.1 200"), "got: {response}");
        }

        // Served one after the other, two 300 ms requests would take 600 ms.
        let elapsed = started.elapsed();
        assert!(elapsed < Duration::from_millis(550), "took {elapsed:?}");
    }
}
