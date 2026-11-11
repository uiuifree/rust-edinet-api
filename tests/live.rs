//! Tests against the real EDINET API.
//!
//! They are ignored by default because they need an API key. Run them one at
//! a time to keep the request rate low:
//!
//! ```bash
//! EDINET_API_KEY=... cargo test --test live -- --ignored --test-threads 1
//! ```

use edinet_api::{Date, DocumentKind, EdinetApi, FileFormat};

fn api() -> EdinetApi {
    let key = std::env::var("EDINET_API_KEY").expect("EDINET_API_KEY is not set");
    EdinetApi::with_api_key(key)
}

/// A weekday with filings that stays inside the 10 year window for years.
fn date() -> Date {
    "2025-06-27".parse().unwrap()
}

#[tokio::test]
#[ignore]
async fn metadata_and_list_agree() {
    let api = api();
    let metadata = api.metadata(date()).await.unwrap().metadata;
    assert!(metadata.resultset.count > 0);
    assert_eq!(metadata.parameter.date, date());

    let list = api.documents(date()).await.unwrap();
    assert_eq!(list.results.len() as u32, list.metadata.resultset.count);
    assert!(list.results.iter().any(|doc| doc.xbrl_flag == Some(true)));
}

#[tokio::test]
#[ignore]
async fn downloads_pdf_and_zip() {
    let api = api();
    let list = api.documents(date()).await.unwrap();
    let doc = list
        .results
        .iter()
        .find(|doc| doc.pdf_flag == Some(true) && doc.xbrl_flag == Some(true))
        .expect("a document with PDF and XBRL");

    let pdf = api.download(&doc.doc_id, DocumentKind::Pdf).await.unwrap();
    assert_eq!(pdf.format, FileFormat::Pdf);
    assert!(pdf.bytes.starts_with(b"%PDF"));

    let zip = api.download(&doc.doc_id, DocumentKind::Main).await.unwrap();
    assert_eq!(zip.format, FileFormat::Zip);
    assert!(zip.bytes.starts_with(b"PK"));
}

#[tokio::test]
#[ignore]
async fn invalid_key_is_unauthorized() {
    let error = EdinetApi::with_api_key("invalid")
        .metadata(date())
        .await
        .unwrap_err();
    assert!(error.is_unauthorized(), "{error}");
}

#[tokio::test]
#[ignore]
async fn unknown_document_is_not_found() {
    let error = api()
        .download("S0000000", DocumentKind::Main)
        .await
        .unwrap_err();
    assert!(error.is_not_found(), "{error}");
}
