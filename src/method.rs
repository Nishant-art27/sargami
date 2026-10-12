use std::fmt;
use std::str::FromStr;

/// An HTTP request method.
#[derive(Hash, Eq, PartialEq, Debug, Clone, Copy)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl Method {
    /// The method's name as it appears on the wire, e.g. `"GET"`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
            Method::Head => "HEAD",
            Method::Options => "OPTIONS",
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The request line named a method this server does not know.
#[derive(Debug, PartialEq, Eq)]
pub struct UnknownMethod(pub String);

impl fmt::Display for UnknownMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown HTTP method: {:?}", self.0)
    }
}

impl std::error::Error for UnknownMethod {}

impl FromStr for Method {
    type Err = UnknownMethod;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GET" => Ok(Method::Get),
            "POST" => Ok(Method::Post),
            "PUT" => Ok(Method::Put),
            "PATCH" => Ok(Method::Patch),
            "DELETE" => Ok(Method::Delete),
            "HEAD" => Ok(Method::Head),
            "OPTIONS" => Ok(Method::Options),
            _ => Err(UnknownMethod(s.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_its_wire_name() {
        let all = [
            Method::Get,
            Method::Post,
            Method::Put,
            Method::Patch,
            Method::Delete,
            Method::Head,
            Method::Options,
        ];
        for method in all {
            assert_eq!(method.to_string().parse::<Method>(), Ok(method));
        }
    }

    #[test]
    fn unknown_method_is_an_error() {
        assert_eq!(
            "BREW".parse::<Method>(),
            Err(UnknownMethod("BREW".to_string()))
        );
    }
}
