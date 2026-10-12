use std::collections::HashMap;
use std::fmt;

/// Why a raw HTTP request could not be turned into a [`Req`].
#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    /// The request contained no data at all.
    Empty,
    /// The request line was not `METHOD PATH VERSION`.
    MalformedRequestLine(String),
    /// A header line had no `:` separator.
    MalformedHeader(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Empty => write!(f, "empty request"),
            ParseError::MalformedRequestLine(line) => {
                write!(f, "malformed request line: {:?}", line)
            }
            ParseError::MalformedHeader(line) => write!(f, "malformed header: {:?}", line),
        }
    }
}

impl std::error::Error for ParseError {}

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
    /// Parse a raw HTTP request (request line plus headers) into a `Req`.
    ///
    /// Returns an error instead of panicking on empty or malformed input, so a
    /// misbehaving client can never bring the server down.
    pub fn parse(http_request: &str) -> Result<Self, ParseError> {
        let mut lines = http_request.lines();
        let first_line = lines.next().ok_or(ParseError::Empty)?;

        let mut parts = first_line.split_whitespace();
        let (method, path, version) = match (parts.next(), parts.next(), parts.next()) {
            (Some(method), Some(path), Some(version)) => {
                (method.to_string(), path.to_string(), version.to_string())
            }
            _ => return Err(ParseError::MalformedRequestLine(first_line.to_string())),
        };

        let mut headers = Vec::new();
        let query = HashMap::new();
        let mut body = None;

        for line in lines {
            if line.is_empty() {
                break;
            }
            let (key, value) = line
                .split_once(':')
                .ok_or_else(|| ParseError::MalformedHeader(line.to_string()))?;
            headers.push((key.trim().to_string(), value.trim().to_string()));
        }

        for header in &headers {
            if header.0.eq_ignore_ascii_case("Content-Length") {
                body = Some("".to_string());
            }
        }

        Ok(Self {
            method,
            path,
            version,
            headers,
            query,
            body,
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_valid_request() {
        let req = Req::parse("GET /sargam HTTP/1.1\nHost: localhost\nX-Custom: a: b").unwrap();
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/sargam");
        assert_eq!(req.version, "HTTP/1.1");
        assert_eq!(req.get_header("host"), Some("localhost"));
        assert_eq!(req.get_header("X-Custom"), Some("a: b"));
    }

    #[test]
    fn empty_request_is_an_error() {
        assert_eq!(Req::parse("").unwrap_err(), ParseError::Empty);
    }

    #[test]
    fn malformed_request_line_is_an_error() {
        assert_eq!(
            Req::parse("GET /only-two").unwrap_err(),
            ParseError::MalformedRequestLine("GET /only-two".to_string())
        );
    }

    #[test]
    fn header_without_colon_is_an_error() {
        assert_eq!(
            Req::parse("GET / HTTP/1.1\nnot a header").unwrap_err(),
            ParseError::MalformedHeader("not a header".to_string())
        );
    }
}
