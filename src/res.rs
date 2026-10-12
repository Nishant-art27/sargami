use std::fmt;

pub struct Res {
    pub status_code: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Res {
    /// Create a new response with status code and text
    pub fn new(status_code: u16, status_text: &str) -> Self {
        Self {
            status_code,
            status_text: status_text.to_string(),
            headers: Vec::new(),
            body: String::new(),
        }
    }

    pub fn to_string(&self) -> String {
        let headers: String = self
            .headers
            .iter()
            .map(|(k, v)| format!("{}: {}\r\n", k, v))
            .collect();

        format!(
            "HTTP/1.1 {} {}\r\n{}Content-Length: {}\r\n\r\n{}",
            self.status_code,
            self.status_text,
            headers,
            self.body.len(),
            self.body
        )
    }

    /// Create a 200 OK response
    pub fn ok() -> Self {
        Self::new(200, "OK")
    }

    /// Create a 404 Not Found response
    pub fn not_found() -> Self {
        Self::new(404, "Not Found")
    }

    /// Set the response body (builder pattern)
    pub fn with_body(mut self, body: String) -> Self {
        self.body = body;
        self
    }

    /// Add a header to the response (builder pattern)
    pub fn with_header(mut self, key: String, value: String) -> Self {
        self.headers.push((key, value));
        self
    }

    /// Set JSON body with Content-Type header (builder pattern)
    pub fn with_json_body(self, body: String) -> Self {
        self.with_header("Content-Type".to_string(), "application/json".to_string())
            .with_body(body)
    }
}

impl fmt::Display for Res {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let headers: String = self
            .headers
            .iter()
            .map(|(k, v)| format!("{}: {}\r\n", k, v))
            .collect();

        write!(
            f,
            "HTTP/1.1 {} {}\r\n{}Content-Length: {}\r\n\r\n{}",
            self.status_code,
            self.status_text,
            headers,
            self.body.len(),
            self.body
        )
    }
}
