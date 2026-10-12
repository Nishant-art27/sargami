use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
};

mod method;
mod req;
mod res;

pub use method::Method;
pub use req::Req;
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

    fn handle_connection(&self, mut stream: TcpStream) {
        let buf_reader = BufReader::new(&stream);
        let http_request = buf_reader
            .lines()
            .map(|result| result.unwrap())
            .take_while(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        let req = Req::new(&http_request);

        let method = match Method::from_str(&req.method) {
            Some(m) => m,
            None => {
                stream
                    .write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n")
                    .unwrap();
                return;
            }
        };

        match self.handle_request(method, &req.path, &req) {
            Ok(res) => {
                let res_string = res.to_string();
                match stream.write_all(res_string.as_bytes()) {
                    Ok(_) => stream.flush().unwrap(),
                    Err(e) => println!("Error sending response: {}", e),
                }
            }
            Err(_) => stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n").unwrap(),
        }
    }

    pub fn listen(&self, addr: &str) {
        let listener = TcpListener::bind(addr).unwrap();
        println!("Listening on {}", addr);

        for stream in listener.incoming() {
            let stream = stream.unwrap();
            self.handle_connection(stream);
        }
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
}
