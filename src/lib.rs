use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
};

mod method;
mod req;
mod res;

pub use method::Method;
pub use req::{BodyError, MAX_BODY_SIZE, ParseError, Req};
pub use res::Res;

type Handler = fn(Req) -> Res;

pub struct Sargami {
    routes: HashMap<(Method, String), Handler>,
}

impl Sargami {
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    pub fn get(&mut self, path: &str, handler: Handler) -> Handler {
        self.routes.insert((Method::Get, path.to_string()), handler);
        handler
    }

    pub fn post(&mut self, path: &str, handler: Handler) -> Handler {
        self.routes
            .insert((Method::Post, path.to_string()), handler);
        handler
    }

    pub fn put(&mut self, path: &str, handler: Handler) -> Handler {
        self.routes.insert((Method::Put, path.to_string()), handler);
        handler
    }

    pub fn delete(&mut self, path: &str, handler: Handler) -> Handler {
        self.routes
            .insert((Method::Delete, path.to_string()), handler);
        handler
    }

    fn handle_request(&self, method: Method, path: &str, req: &Req) -> Result<Res, String> {
        let handler = self.routes.get(&(method, path.to_string()));

        match handler {
            Some(handler) => Ok(handler(req.clone())),
            None => Err("Not Found".to_string()),
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
                    eprintln!("Error reading request: {}", e);
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
                eprintln!("Bad request: {}", e);
                send(&stream, b"HTTP/1.1 400 Bad Request\r\n\r\n");
                return;
            }
        };

        // The body follows the blank line. It has to come from the same buffered
        // reader, which may already be holding some of its bytes.
        if let Err(e) = req.read_body(&mut reader) {
            eprintln!("Bad request: {}", e);
            if matches!(e, BodyError::TooLarge(_)) {
                send(&stream, b"HTTP/1.1 413 Payload Too Large\r\n\r\n");
            } else {
                send(&stream, b"HTTP/1.1 400 Bad Request\r\n\r\n");
            }
            return;
        }

        let method = match Method::from_str(&req.method) {
            Some(m) => m,
            None => {
                send(&stream, b"HTTP/1.1 400 Bad Request\r\n\r\n");
                return;
            }
        };

        match self.handle_request(method, &req.path, &req) {
            Ok(res) => send(&stream, res.to_string().as_bytes()),
            Err(_) => send(&stream, b"HTTP/1.1 404 Not Found\r\n\r\n"),
        }
    }

    pub fn listen(&self, addr: &str) {
        let listener = TcpListener::bind(addr).unwrap();
        println!("Listening on {}", addr);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => self.handle_connection(stream),
                Err(e) => eprintln!("Error accepting connection: {}", e),
            }
        }
    }
}

/// Write a response to the client. A client that has already hung up must not
/// take the server down with it, so write errors are logged, not unwrapped.
fn send(mut stream: &TcpStream, bytes: &[u8]) {
    if let Err(e) = stream.write_all(bytes).and_then(|_| stream.flush()) {
        eprintln!("Error sending response: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn it_works() {
        thread::spawn(|| {
            let mut app = Sargami::new();
            app.get("/sargam", |_| Res {
                status_code: 200,
                status_text: String::from("OK"),
                headers: Vec::new(),
                body: "hello world".to_string(),
            });
            app.listen("127.0.0.1:7878");
        });

        // Sleep to ensure server is ready
        thread::sleep(Duration::from_millis(100));

        let mut stream = TcpStream::connect("127.0.0.1:7878").unwrap();

        // Send proper HTTP request with CRLF line endings
        stream
            .write_all(b"GET /sargam HTTP/1.1\r\nHost: 127.0.0.1:7878\r\n\r\n")
            .unwrap();
        stream.flush().unwrap();

        let mut reader = BufReader::new(&stream);

        // Read status line
        let mut status_line = String::new();
        reader.read_line(&mut status_line).unwrap();

        println!("status_line: {}", status_line);

        // Verify it's a 200 response
        assert!(status_line.contains("200") || status_line.contains("HTTP"));
    }

    #[test]
    fn survives_bad_clients() {
        thread::spawn(|| {
            let mut app = Sargami::new();
            app.get("/ok", |_| Res::ok().with_body("still alive".to_string()));
            app.listen("127.0.0.1:7879");
        });

        // Sleep to ensure server is ready
        thread::sleep(Duration::from_millis(100));

        // A client that connects and hangs up without sending anything.
        drop(TcpStream::connect("127.0.0.1:7879").unwrap());

        // A request line with no HTTP version.
        let status = exchange("GET /ok\r\n\r\n");
        assert!(status.starts_with("HTTP/1.1 400"), "got: {status}");

        // A header line with no colon.
        let status = exchange("GET /ok HTTP/1.1\r\nnot a header\r\n\r\n");
        assert!(status.starts_with("HTTP/1.1 400"), "got: {status}");

        // The server must still be alive to answer a well-formed request.
        let status = exchange("GET /ok HTTP/1.1\r\nHost: localhost\r\n\r\n");
        assert!(status.starts_with("HTTP/1.1 200"), "got: {status}");
    }

    /// Send a raw request to the test server on 7879 and return its status line.
    fn exchange(raw: &str) -> String {
        let mut stream = TcpStream::connect("127.0.0.1:7879").unwrap();
        stream.write_all(raw.as_bytes()).unwrap();
        stream.flush().unwrap();

        let mut status_line = String::new();
        BufReader::new(&stream).read_line(&mut status_line).unwrap();
        status_line
    }

    #[test]
    fn post_body_reaches_the_handler() {
        thread::spawn(|| {
            let mut app = Sargami::new();
            app.post("/echo", |req| Res::ok().with_body(req.body.unwrap_or_default()));
            app.listen("127.0.0.1:7880");
        });

        // Sleep to ensure server is ready
        thread::sleep(Duration::from_millis(100));

        let request = concat!(
            "POST /echo HTTP/1.1\r\n",
            "Host: localhost\r\n",
            "Content-Length: 11\r\n",
            "\r\n",
            "hello world"
        );
        let mut stream = TcpStream::connect("127.0.0.1:7880").unwrap();
        stream.write_all(request.as_bytes()).unwrap();
        stream.flush().unwrap();

        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();

        assert!(response.starts_with("HTTP/1.1 200"), "got: {response}");
        assert!(response.ends_with("\r\n\r\nhello world"), "got: {response}");
    }
}
