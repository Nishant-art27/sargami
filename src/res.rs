use std::fmt;

/// An HTTP response. Build one with [`Res::ok`] or [`Res::new`] and the
/// `with_*` methods, then return it from a handler.
#[derive(Debug, Clone)]
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

    /// Create a 200 OK response
    pub fn ok() -> Self {
        Self::new(200, "OK")
    }

    /// Create a 400 Bad Request response
    pub fn bad_request() -> Self {
        Self::new(400, "Bad Request")
    }

    /// Create a 404 Not Found response
    pub fn not_found() -> Self {
        Self::new(404, "Not Found")
    }

    /// Create a 413 Payload Too Large response
    pub fn payload_too_large() -> Self {
        Self::new(413, "Payload Too Large")
    }

    /// Set the response body (builder pattern)
    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    /// Add a header to the response (builder pattern)
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((key.into(), value.into()));
        self
    }

    /// Set JSON body with Content-Type header (builder pattern)
    pub fn with_json_body(self, body: impl Into<String>) -> Self {
        self.with_header("Content-Type", "application/json")
            .with_body(body)
    }
}

/// Serializes the response in HTTP/1.1 wire format. `Content-Length` is always
/// derived from the body, and `Connection: close` is sent because the server
/// closes every connection after one response.
impl fmt::Display for Res {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HTTP/1.1 {} {}", self.status_code, self.status_text)?;
        for (key, value) in &self.headers {
            write!(f, "\r\n{key}: {value}")?;
        }
        write!(f, "\r\nContent-Length: {}", self.body.len())?;
        write!(f, "\r\nConnection: close\r\n\r\n{}", self.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_http_wire_format() {
        let res = Res::ok().with_json_body(r#"{"ok":true}"#);
        let expected = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Type: application/json\r\n",
            "Content-Length: 11\r\n",
            "Connection: close\r\n",
            "\r\n",
            r#"{"ok":true}"#
        );
        assert_eq!(res.to_string(), expected);
    }

    #[test]
    fn status_helpers_set_code_and_text() {
        assert_eq!(Res::not_found().status_code, 404);
        assert_eq!(Res::bad_request().status_text, "Bad Request");
        assert_eq!(Res::payload_too_large().status_code, 413);
    }
}
