use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Req {
    pub method: String,
    pub path: String,
    pub version: String,
    pub headers: Vec<(String, String)>,
    pub query: HashMap<String, String>,
    pub body: Option<String>,
}

impl Req {
    pub fn new(http_request: &str) -> Self {
        let mut lines = http_request.lines();
        let first_line = lines.next().unwrap();
        let mut parts = first_line.split_whitespace();
        let method = parts.next().unwrap().to_string();
        let path = parts.next().unwrap().to_string();
        let version = parts.next().unwrap().to_string();

        let mut headers = Vec::new();
        let mut query = HashMap::new();
        let mut body = None;

        for line in lines {
            if line.is_empty() {
                break;
            }
            let mut parts = line.split(": ");
            let key = parts.next().unwrap().to_string();
            let value = parts.next().unwrap().to_string();
            headers.push((key, value));
        }

        for header in &headers {
            if header.0.eq_ignore_ascii_case("Content-Length") {
                body = Some("".to_string());
            }
        }

        Self {
            method,
            path,
            version,
            headers,
            query,
            body,
        }
    }

    /// Get a header value by key (case-insensitive)
    pub fn get_header(&self, key: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    /// Get a query parameter value by key
    pub fn get_query(&self, key: &str) -> Option<&str> {
        self.query.get(key).map(|v| v.as_str())
    }
}
