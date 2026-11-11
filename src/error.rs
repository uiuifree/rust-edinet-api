use serde_json::Value;
use std::error::Error;
use std::fmt::{Display, Formatter};

/// Error type returned by all API calls.
///
/// EDINET reports most errors with HTTP status `200` and a JSON body, so
/// [`Api`](Self::Api) holds the status from that body, not the HTTP status.
///
/// # Example
///
/// ```
/// use edinet_api::EdinetError;
///
/// let error = EdinetError::Api { status: 404, message: "Not Found".into() };
/// assert!(error.is_not_found());
/// assert_eq!(error.status(), Some(404));
/// ```
#[derive(Debug)]
#[non_exhaustive]
pub enum EdinetError {
    /// Failed to connect, send the request, or read the response body.
    ///
    /// The request URL is removed from the error because it contains the API key.
    Connection(reqwest::Error),
    /// The response body is not the expected JSON.
    JsonParse(serde_json::Error),
    /// EDINET returned an error in the response body.
    ///
    /// `400` invalid parameters, `401` missing or invalid API key, `404` no
    /// such date or document, `429` too many requests, `500` server error.
    Api {
        /// Status from the body (`metadata.status` or `StatusCode`).
        status: u16,
        /// Message from the body, such as `Not Found`.
        message: String,
    },
    /// The server answered with an HTTP error status and a body that is not
    /// an EDINET error, such as a maintenance page. Holds the HTTP status and the body.
    HttpStatus(u16, String),
    /// The document download returned a `Content-Type` that is neither ZIP nor PDF.
    UnexpectedContentType(String),
    /// The request was rejected before sending.
    InvalidArgument(String),
}

impl EdinetError {
    /// Status of [`Api`](Self::Api) or [`HttpStatus`](Self::HttpStatus), `None` otherwise.
    pub fn status(&self) -> Option<u16> {
        match self {
            EdinetError::Api { status, .. } | EdinetError::HttpStatus(status, _) => Some(*status),
            _ => None,
        }
    }

    /// `true` for status `404`: the date is out of range or the document
    /// cannot be downloaded (withdrawn, past its inspection period, or not disclosed).
    pub fn is_not_found(&self) -> bool {
        self.status() == Some(404)
    }

    /// `true` for status `401`: the API key is missing or invalid.
    pub fn is_unauthorized(&self) -> bool {
        self.status() == Some(401)
    }

    /// `true` for status `429`: too many requests in a short time.
    pub fn is_rate_limited(&self) -> bool {
        self.status() == Some(429)
    }

    /// `true` when the request timed out.
    pub fn is_timeout(&self) -> bool {
        matches!(self, EdinetError::Connection(e) if e.is_timeout())
    }

    pub(crate) fn connection(error: reqwest::Error) -> Self {
        EdinetError::Connection(error.without_url())
    }

    /// Reads an EDINET error from a response body. `None` when the body is
    /// not an error, including a successful document list.
    pub(crate) fn from_body(body: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(body).ok()?;
        // 401 and 429 come from the API gateway in a different shape
        if let Some(status) = value.get("StatusCode").and_then(Value::as_u64) {
            return Some(EdinetError::Api {
                status: u16::try_from(status).ok()?,
                message: message(&value),
            });
        }
        let metadata = value.get("metadata")?;
        let status = metadata.get("status")?.as_str()?;
        if status == "200" {
            return None;
        }
        Some(EdinetError::Api {
            status: status.parse().ok()?,
            message: message(metadata),
        })
    }
}

fn message(value: &Value) -> String {
    value
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

impl Display for EdinetError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EdinetError::Connection(e) => write!(f, "connection error: {e}"),
            EdinetError::JsonParse(e) => write!(f, "json parse error: {e}"),
            EdinetError::Api { status, message } => write!(f, "edinet error {status}: {message}"),
            EdinetError::HttpStatus(status, body) => write!(f, "http status {status}: {body}"),
            EdinetError::UnexpectedContentType(content_type) => {
                write!(f, "unexpected content type: {content_type}")
            }
            EdinetError::InvalidArgument(e) => write!(f, "invalid argument: {e}"),
        }
    }
}

impl Error for EdinetError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            EdinetError::Connection(e) => Some(e),
            EdinetError::JsonParse(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status_of(body: &str) -> Option<u16> {
        EdinetError::from_body(body)?.status()
    }

    #[test]
    fn reads_metadata_errors() {
        let error = EdinetError::from_body(
            r#"{"metadata":{"title":"提出された書類を把握するためのAPI","status":"404","message":"Not Found"}}"#,
        )
        .unwrap();
        assert!(error.is_not_found());
        assert_eq!(error.to_string(), "edinet error 404: Not Found");
    }

    #[test]
    fn reads_gateway_errors() {
        let error =
            EdinetError::from_body(r#"{"StatusCode": 401,"message": "Access denied"}"#).unwrap();
        assert!(error.is_unauthorized());
        assert!(!error.is_rate_limited());
        let error = EdinetError::from_body(r#"{"StatusCode": 429}"#).unwrap();
        assert!(error.is_rate_limited());
        assert_eq!(error.to_string(), "edinet error 429: ");
    }

    #[test]
    fn reads_status_from_the_body() {
        assert_eq!(
            status_of(r#"{"metadata":{"status":"500","message":"Internal Server Error"}}"#),
            Some(500)
        );
    }

    #[test]
    fn ignores_bodies_that_are_not_errors() {
        assert_eq!(
            status_of(r#"{"metadata":{"status":"200","message":"OK"}}"#),
            None
        );
        assert_eq!(status_of("<html>sorry</html>"), None);
        assert_eq!(status_of(r#"{"results":[]}"#), None);
        assert_eq!(status_of(r#"{"metadata":{"message":"OK"}}"#), None);
        assert_eq!(status_of(r#"{"metadata":{"status":404}}"#), None);
        assert_eq!(status_of(r#"{"metadata":{"status":"error"}}"#), None);
        assert_eq!(status_of(r#"{"StatusCode": 70000}"#), None);
    }

    #[test]
    fn connection_errors() {
        // A builder error is the only reqwest::Error that can be made without I/O
        let request = reqwest::Client::new().get("not a url").build();
        let error = EdinetError::connection(request.unwrap_err());
        assert!(!error.is_timeout());
        assert!(error.to_string().starts_with("connection error: "));
        assert!(error.source().is_some());
        assert_eq!(error.status(), None);
    }

    #[test]
    fn display_and_source() {
        let json = serde_json::from_str::<Value>("{").unwrap_err();
        let error = EdinetError::JsonParse(json);
        assert!(error.to_string().starts_with("json parse error: "));
        assert!(error.source().is_some());
        assert_eq!(error.status(), None);

        let error = EdinetError::HttpStatus(503, "maintenance".into());
        assert_eq!(error.to_string(), "http status 503: maintenance");
        assert_eq!(error.status(), Some(503));
        assert!(error.source().is_none());
        assert!(!error.is_timeout());

        let error = EdinetError::UnexpectedContentType("text/html".into());
        assert_eq!(error.to_string(), "unexpected content type: text/html");
        let error = EdinetError::InvalidArgument("bad".into());
        assert_eq!(error.to_string(), "invalid argument: bad");
    }
}
