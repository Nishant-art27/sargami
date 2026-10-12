use std::collections::HashMap;
use std::fmt;
use std::io::{self, Read};

/// Largest request body the server will read, in bytes. Anything bigger is
/// rejected before a buffer is allocated for it.
pub const MAX_BODY_SIZE: usize = 1024 * 1024;

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

/// Why a request body could not be read.
#[derive(Debug)]
pub enum BodyError {
    /// The Content-Length header is not a valid number.
    BadContentLength(String),
    /// The declared Content-Length exceeds [`MAX_BODY_SIZE`].
    TooLarge(usize),
    /// The connection failed or closed before the whole body arrived.
    Io(io::Error),
}

impl fmt::Display for BodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BodyError::BadContentLength(raw) => write!(f, "invalid Content-Length: {:?}", raw),
            BodyError::TooLarge(len) => {
                write!(f, "body of {} bytes exceeds the {} byte limit", len, MAX_BODY_SIZE)
            }
            BodyError::Io(e) => write!(f, "could not read body: {}", e),
        }
    }
}

impl std::error::Error for BodyError {}

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
    /// Parse the head of a raw HTTP request (request line plus headers).
    ///
    /// The body is not part of the head; call [`Req::read_body`] afterwards to
    /// read it from the connection.
    ///
    /// Returns an error instead of panicking on empty or malformed input, so a
    /// misbehaving client can never bring the server down.
    pub fn parse(http_request: &str) -> Result<Self, ParseError> {
        let mut lines = http_request.lines();
        let first_line = lines.next().ok_or(ParseError::Empty)?;

        let mut parts = first_line.split_whitespace();
        let (method, target, version) = match (parts.next(), parts.next(), parts.next()) {
            (Some(method), Some(target), Some(version)) => (method, target, version),
            _ => return Err(ParseError::MalformedRequestLine(first_line.to_string())),
        };

        // Everything after the first `?` is the query string, not the path, so
        // `/users?id=1` must still match a route registered as `/users`.
        let (path, query) = match target.split_once('?') {
            Some((path, raw_query)) => (path, parse_query(raw_query)),
            None => (target, HashMap::new()),
        };

        let mut headers = Vec::new();
        for line in lines {
            if line.is_empty() {
                break;
            }
            let (key, value) = line
                .split_once(':')
                .ok_or_else(|| ParseError::MalformedHeader(line.to_string()))?;
            headers.push((key.trim().to_string(), value.trim().to_string()));
        }

        Ok(Self {
            method: method.to_string(),
            path: path.to_string(),
            version: version.to_string(),
            headers,
            query,
            body: None,
        })
    }

    /// Read the body declared by the Content-Length header from `reader` into
    /// `self.body`. A request without a Content-Length header is left with no
    /// body.
    pub fn read_body(&mut self, reader: &mut impl Read) -> Result<(), BodyError> {
        let Some(raw) = self.get_header("Content-Length") else {
            return Ok(());
        };

        let len: usize = raw
            .parse()
            .map_err(|_| BodyError::BadContentLength(raw.to_string()))?;
        if len > MAX_BODY_SIZE {
            return Err(BodyError::TooLarge(len));
        }

        let mut bytes = vec![0; len];
        reader.read_exact(&mut bytes).map_err(BodyError::Io)?;
        self.body = Some(String::from_utf8_lossy(&bytes).into_owned());
        Ok(())
    }

    /// Get a header value by key (case-insensitive)
    pub fn get_header(&self, key: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    /// Get a query parameter value by key. Values are already percent-decoded.
    pub fn get_query(&self, key: &str) -> Option<&str> {
        self.query.get(key).map(|v| v.as_str())
    }
}

/// Parse `a=1&b=hello%20world` into a map. Keys and values are percent-decoded,
/// `+` means space, a key with no `=` gets an empty value, and when a key
/// repeats the last value wins.
fn parse_query(raw: &str) -> HashMap<String, String> {
    raw.split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (percent_decode(key), percent_decode(value))
        })
        .collect()
}

/// Decode `%XX` escapes and `+` as used in URL query strings. A malformed
/// escape such as `%zz` is passed through unchanged.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' => match decode_escape(bytes, i) {
                Some(byte) => {
                    out.push(byte);
                    i += 2; // skip the two hex digits as well as the '%'
                }
                None => out.push(b'%'),
            },
            other => out.push(other),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The byte encoded by the `%XX` escape starting at `bytes[i]`, if it is valid.
fn decode_escape(bytes: &[u8], i: usize) -> Option<u8> {
    let hi = (bytes.get(i + 1).copied()? as char).to_digit(16)?;
    let lo = (bytes.get(i + 2).copied()? as char).to_digit(16)?;
    Some((hi * 16 + lo) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

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

    #[test]
    fn reads_exactly_content_length_bytes() {
        let mut req = Req::parse("POST / HTTP/1.1\nContent-Length: 5").unwrap();
        req.read_body(&mut Cursor::new(&b"hello, and more"[..])).unwrap();
        assert_eq!(req.body.as_deref(), Some("hello"));
    }

    #[test]
    fn no_content_length_means_no_body() {
        let mut req = Req::parse("GET / HTTP/1.1\nHost: localhost").unwrap();
        req.read_body(&mut Cursor::new(&b""[..])).unwrap();
        assert_eq!(req.body, None);
    }

    #[test]
    fn invalid_content_length_is_an_error() {
        let mut req = Req::parse("POST / HTTP/1.1\nContent-Length: lots").unwrap();
        let err = req.read_body(&mut Cursor::new(&b""[..])).unwrap_err();
        assert!(matches!(err, BodyError::BadContentLength(_)), "{err:?}");
    }

    #[test]
    fn oversized_body_is_rejected_before_reading() {
        let head = format!("POST / HTTP/1.1\nContent-Length: {}", MAX_BODY_SIZE + 1);
        let mut req = Req::parse(&head).unwrap();
        let err = req.read_body(&mut Cursor::new(&b""[..])).unwrap_err();
        assert!(matches!(err, BodyError::TooLarge(_)), "{err:?}");
    }

    #[test]
    fn truncated_body_is_an_error() {
        let mut req = Req::parse("POST / HTTP/1.1\nContent-Length: 10").unwrap();
        let err = req.read_body(&mut Cursor::new(&b"short"[..])).unwrap_err();
        assert!(matches!(err, BodyError::Io(_)), "{err:?}");
    }

    #[test]
    fn splits_query_string_off_the_path() {
        let req = Req::parse("GET /users?id=7&name=Ada+Lovelace&tag=a%2Cb HTTP/1.1").unwrap();
        assert_eq!(req.path, "/users");
        assert_eq!(req.get_query("id"), Some("7"));
        assert_eq!(req.get_query("name"), Some("Ada Lovelace"));
        assert_eq!(req.get_query("tag"), Some("a,b"));
        assert_eq!(req.get_query("missing"), None);
    }

    #[test]
    fn query_edge_cases() {
        let req = Req::parse("GET /p?flag&empty=&a=1&a=2&&x=%zz HTTP/1.1").unwrap();
        assert_eq!(req.path, "/p");
        assert_eq!(req.get_query("flag"), Some(""));
        assert_eq!(req.get_query("empty"), Some(""));
        assert_eq!(req.get_query("a"), Some("2"));
        assert_eq!(req.get_query("x"), Some("%zz"));
        assert_eq!(req.query.len(), 4);
    }

    #[test]
    fn path_without_query_has_empty_query_map() {
        let req = Req::parse("GET /plain HTTP/1.1").unwrap();
        assert_eq!(req.path, "/plain");
        assert!(req.query.is_empty());
    }
}
