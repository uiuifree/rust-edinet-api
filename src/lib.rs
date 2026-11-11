#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

pub mod code;
mod date;
mod download;
mod error;
pub mod response;

pub use date::{Date, DateTime};
pub use download::{DocumentKind, Download, FileFormat};
pub use error::EdinetError;
pub use response::{Document, DocumentList, Metadata, MetadataResponse};

/// Re-export of `reqwest`, for building a client to pass to
/// [`EdinetApi::with_client`] without adding the dependency yourself.
pub use reqwest;

use reqwest::header::CONTENT_TYPE;
use reqwest::{Response, Url};
use serde::de::DeserializeOwned;
use std::fmt::{Debug, Formatter};
use std::time::Duration;

const DEFAULT_BASE_URL: &str = "https://api.edinet-fsa.go.jp/api/v2";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Client for the EDINET API v2.
///
/// The API key is issued on the EDINET website and sent as the
/// `Subscription-Key` query parameter, as the specification requires.
///
/// # Example
///
/// ```no_run
/// use edinet_api::code::DocTypeCode;
/// use edinet_api::{DocumentKind, EdinetApi};
///
/// # async fn run() -> Result<(), edinet_api::EdinetError> {
/// let api = EdinetApi::with_api_key("YOUR_API_KEY");
/// let list = api.documents("2026-06-30".parse()?).await?;
///
/// for doc in &list.results {
///     if doc.doc_type_code == Some(DocTypeCode::AnnualSecuritiesReport) && doc.xbrl_flag == Some(true) {
///         let zip = api.download(&doc.doc_id, DocumentKind::Main).await?;
///         std::fs::write(format!("{}.zip", doc.doc_id), &zip.bytes).unwrap();
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct EdinetApi {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
    timeout: Option<Duration>,
}

impl Debug for EdinetApi {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EdinetApi")
            .field("api_key", &"<redacted>")
            .field("base_url", &self.base_url)
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl EdinetApi {
    /// Client with the given API key. Requests time out after 60 seconds.
    pub fn with_api_key<T: Into<String>>(key: T) -> Self {
        EdinetApi {
            api_key: key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            client: reqwest::Client::new(),
            timeout: Some(DEFAULT_TIMEOUT),
        }
    }

    /// Uses the given HTTP client, for example one with a proxy. The 60 second
    /// default no longer applies, so set a timeout on the client if you want one.
    pub fn with_client(mut self, client: reqwest::Client) -> Self {
        self.client = client;
        self.timeout = None;
        self
    }

    /// Timeout for each request, including reading the body.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Changes the API base URL (default `https://api.edinet-fsa.go.jp/api/v2`).
    /// Mainly for tests against a mock server.
    pub fn with_base_url<T: Into<String>>(mut self, base_url: T) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }

    /// Metadata of the document list for a date, without the list itself
    /// (`GET documents.json?type=1`). Use it to check whether
    /// `metadata.resultset.count` went up before
    /// fetching the full list.
    ///
    /// The date can be any day, including weekends, up to 10 years back.
    ///
    /// # Errors
    ///
    /// * [`EdinetError::Api`] with `404` for a date out of range (see
    ///   [`EdinetError::is_not_found`]), `401` for a bad API key, `429` when
    ///   rate limited.
    /// * [`EdinetError::Connection`] on a network failure or timeout,
    ///   [`EdinetError::HttpStatus`] when EDINET answers with an error page,
    ///   or [`EdinetError::JsonParse`] when the body is not the expected JSON.
    pub async fn metadata(&self, date: Date) -> Result<MetadataResponse, EdinetError> {
        self.list(date, "1").await
    }

    /// Document list for a date (`GET documents.json?type=2`).
    ///
    /// The list for a date holds the documents submitted that day plus
    /// entries for corrections and non-disclosure registered that day.
    /// Today's list is updated about every minute from 8:30 JST.
    ///
    /// # Errors
    ///
    /// Same as [`metadata`](Self::metadata).
    pub async fn documents(&self, date: Date) -> Result<DocumentList, EdinetError> {
        self.list(date, "2").await
    }

    /// Downloads a document (`GET documents/{doc_id}`). Check the flags of the
    /// [`Document`] entry first: a missing file is reported as `404`.
    ///
    /// # Errors
    ///
    /// * [`EdinetError::Api`] with `404` when the document or the requested
    ///   file does not exist or is no longer available.
    /// * [`EdinetError::InvalidArgument`] when `doc_id` is not alphanumeric.
    /// * [`EdinetError::UnexpectedContentType`] when the body is neither ZIP nor PDF.
    /// * Otherwise the same as [`metadata`](Self::metadata).
    pub async fn download(
        &self,
        doc_id: &str,
        kind: DocumentKind,
    ) -> Result<Download, EdinetError> {
        if doc_id.is_empty() || !doc_id.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(EdinetError::InvalidArgument(format!(
                "document id must be alphanumeric: {doc_id:?}"
            )));
        }
        let url = self.url(&format!("documents/{doc_id}"), &[("type", kind.code())])?;
        let response = self.get(url).await?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let bytes = response.bytes().await.map_err(EdinetError::connection)?;

        let error = || {
            let body = String::from_utf8_lossy(&bytes).into_owned();
            EdinetError::from_body(&body).unwrap_or(EdinetError::HttpStatus(status.as_u16(), body))
        };
        if content_type.starts_with("application/json") || !status.is_success() {
            return Err(error());
        }
        let format = if content_type.starts_with("application/pdf") {
            FileFormat::Pdf
        } else if content_type.starts_with("application/octet-stream")
            || content_type.starts_with("application/zip")
        {
            FileFormat::Zip
        } else {
            return Err(EdinetError::UnexpectedContentType(content_type));
        };
        Ok(Download {
            format,
            bytes: bytes.into(),
        })
    }

    async fn list<T: DeserializeOwned>(
        &self,
        date: Date,
        list_type: &str,
    ) -> Result<T, EdinetError> {
        let date = date.to_string();
        let url = self.url("documents.json", &[("date", &date), ("type", list_type)])?;
        let response = self.get(url).await?;
        let status = response.status();
        let text = response.text().await.map_err(EdinetError::connection)?;
        if let Some(error) = EdinetError::from_body(&text) {
            return Err(error);
        }
        if !status.is_success() {
            return Err(EdinetError::HttpStatus(status.as_u16(), text));
        }
        serde_json::from_str(&text).map_err(EdinetError::JsonParse)
    }

    fn url(&self, path: &str, query: &[(&str, &str)]) -> Result<Url, EdinetError> {
        let mut url = Url::parse(&format!("{}/{}", self.base_url, path))
            .map_err(|e| EdinetError::InvalidArgument(format!("base url: {e}")))?;
        url.query_pairs_mut()
            .extend_pairs(query)
            .append_pair("Subscription-Key", &self.api_key);
        Ok(url)
    }

    async fn get(&self, url: Url) -> Result<Response, EdinetError> {
        let mut request = self.client.get(url);
        if let Some(timeout) = self.timeout {
            request = request.timeout(timeout);
        }
        request.send().await.map_err(EdinetError::connection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_urls() {
        let api = EdinetApi::with_api_key("a&b").with_base_url("https://example.com/api/v2/");
        let url = api
            .url("documents.json", &[("date", "2023-04-03"), ("type", "2")])
            .unwrap();
        assert_eq!(
            url.as_str(),
            "https://example.com/api/v2/documents.json?date=2023-04-03&type=2&Subscription-Key=a%26b"
        );
        let url = EdinetApi::with_api_key("k")
            .url("documents/S1000001", &[("type", "1")])
            .unwrap();
        assert_eq!(
            url.as_str(),
            "https://api.edinet-fsa.go.jp/api/v2/documents/S1000001?type=1&Subscription-Key=k"
        );
    }

    #[test]
    fn rejects_bad_base_url() {
        let api = EdinetApi::with_api_key("k").with_base_url("not a url");
        let error = api.url("documents.json", &[]).unwrap_err();
        assert!(error.to_string().starts_with("invalid argument: base url"));
    }

    #[test]
    fn debug_hides_the_key() {
        let api = EdinetApi::with_api_key("secret").with_timeout(Duration::from_secs(5));
        let debug = format!("{api:?}");
        assert!(!debug.contains("secret"));
        assert!(debug.contains("5s"));
        let debug = format!("{:?}", api.with_client(reqwest::Client::new()));
        assert!(debug.contains("timeout: None"));
    }
}
