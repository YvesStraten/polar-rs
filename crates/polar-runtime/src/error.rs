#[derive(Debug)]
pub enum Error {
    Transport(reqwest::Error),
    Decode(serde_json::Error),
    Api { status: u16, body: String },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Transport(e) => write!(f, "transport error: {e}"),
            Error::Decode(e) => write!(f, "decode error: {e}"),
            Error::Api { status, body } => write!(f, "API error {status}: {body}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Transport(e) => Some(e),
            Error::Decode(e) => Some(e),
            Error::Api { .. } => None,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Transport(e)
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Decode(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn api_error_displays_status_and_body() {
        let e = Error::Api {
            status: 404,
            body: "not found".into(),
        };
        assert_eq!(e.to_string(), "API error 404: not found");
    }
}
