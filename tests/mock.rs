use edinet_api::code::{
    DisclosureStatus, DocInfoEditStatus, DocTypeCode, LegalStatus, OrdinanceCode, WithdrawalStatus,
};
use edinet_api::{Date, DocumentKind, EdinetApi, EdinetError, FileFormat};
use httpmock::prelude::*;
use std::error::Error;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const METADATA: &str = include_str!("fixtures/metadata.json");
const DOCUMENTS: &str = include_str!("fixtures/documents.json");
const NOT_FOUND: &str = r#"{"metadata":{"title":"提出された書類を把握するためのAPI","status":"404","message":"Not Found"}}"#;
const BAD_KEY: &str = r#"{"StatusCode": 401,"message": "Access denied due to invalid subscription key.Make sure to provide a valid key for an active subscription."}"#;

fn date(s: &str) -> Date {
    s.parse().unwrap()
}

fn api(server: &MockServer) -> EdinetApi {
    EdinetApi::with_api_key("test-key").with_base_url(server.base_url())
}

#[tokio::test]
async fn metadata_sends_type_1() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/documents.json")
                .query_param("date", "2023-04-03")
                .query_param("type", "1")
                .query_param("Subscription-Key", "test-key");
            then.status(200)
                .header("content-type", "application/json; charset=utf-8")
                .body(METADATA);
        })
        .await;

    let metadata = api(&server).metadata(date("2023-04-03")).await.unwrap();
    mock.assert_async().await;
    assert_eq!(metadata.resultset.count, 1);
    assert_eq!(metadata.parameter.date, date("2023-04-03"));
    assert_eq!(metadata.parameter.list_type, "1");
    assert_eq!(metadata.process_date_time.to_string(), "2023-04-03 13:01");
    assert_eq!(metadata.status, "200");
    assert_eq!(metadata.message, "OK");
    assert_eq!(metadata.title, "提出された書類を把握するためのAPI");
}

#[tokio::test]
async fn documents_parses_entries() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/documents.json")
                .query_param("date", "2023-06-13")
                .query_param("type", "2")
                .query_param("Subscription-Key", "test-key");
            then.status(200)
                .header("content-type", "application/json; charset=utf-8")
                .body(DOCUMENTS);
        })
        .await;

    let list = api(&server).documents(date("2023-06-13")).await.unwrap();
    mock.assert_async().await;
    assert_eq!(list.metadata.resultset.count, 3);
    assert_eq!(list.results.len(), 3);

    let report = &list.results[0];
    assert_eq!(report.seq_number, 1);
    assert_eq!(report.doc_id, "S1000001");
    assert_eq!(report.edinet_code.as_deref(), Some("E10001"));
    assert_eq!(report.jcn.as_deref(), Some("6000012010023"));
    assert_eq!(
        report.ordinance_code,
        Some(OrdinanceCode::SpecifiedSecurities)
    );
    assert_eq!(
        report.doc_type_code,
        Some(DocTypeCode::AnnualSecuritiesReport)
    );
    assert_eq!(report.period_start, Some(date("2022-04-01")));
    assert_eq!(report.period_end, Some(date("2023-03-31")));
    assert_eq!(report.submit_date_time.unwrap().hour(), 12);
    assert_eq!(report.legal_status, LegalStatus::UnderInspection);
    assert!(report.xbrl_flag && report.pdf_flag && report.attach_doc_flag && report.csv_flag);
    assert!(!report.english_doc_flag);

    let withdrawn = &list.results[1];
    assert_eq!(withdrawn.withdrawal_status, WithdrawalStatus::Withdrawn);
    assert_eq!(withdrawn.parent_doc_id.as_deref(), Some("S1000001"));
    assert_eq!(withdrawn.filer_name, None);
    assert_eq!(withdrawn.legal_status, LegalStatus::Expired);

    let extraordinary = &list.results[2];
    assert_eq!(
        extraordinary.doc_info_edit_status,
        DocInfoEditStatus::EditNotice
    );
    assert_eq!(extraordinary.disclosure_status, DisclosureStatus::Disclosed);
    assert_eq!(
        extraordinary.subsidiary_edinet_codes(),
        ["E10003", "E10004"]
    );
    assert_eq!(extraordinary.current_report_reasons().len(), 2);
    assert_eq!(
        extraordinary.ope_date_time.unwrap().to_string(),
        "2023-06-13 17:30"
    );

    // Serialize gives back the wire format
    let value = serde_json::to_value(&list).unwrap();
    let original: serde_json::Value = serde_json::from_str(DOCUMENTS).unwrap();
    assert_eq!(value, original);
}

#[tokio::test]
async fn list_errors_come_with_status_200() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.query_param("date", "2000-01-01");
            then.status(200).body(NOT_FOUND);
        })
        .await;
    server
        .mock_async(|when, then| {
            when.query_param("date", "2023-04-04");
            then.status(200).body(BAD_KEY);
        })
        .await;

    let error = api(&server)
        .documents(date("2000-01-01"))
        .await
        .unwrap_err();
    assert!(error.is_not_found(), "{error}");
    let error = api(&server).metadata(date("2023-04-04")).await.unwrap_err();
    assert!(error.is_unauthorized(), "{error}");
    assert!(error.to_string().contains("invalid subscription key"));
}

#[tokio::test]
async fn list_other_failures() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.query_param("date", "2023-04-05");
            then.status(503).body("<html>Sorry</html>");
        })
        .await;
    server
        .mock_async(|when, then| {
            when.query_param("date", "2023-04-06");
            then.status(200).body(r#"{"metadata":{"status":"200"}}"#);
        })
        .await;

    let error = api(&server)
        .documents(date("2023-04-05"))
        .await
        .unwrap_err();
    assert!(matches!(&error, EdinetError::HttpStatus(503, body) if body.contains("Sorry")));
    let error = api(&server)
        .documents(date("2023-04-06"))
        .await
        .unwrap_err();
    assert!(matches!(error, EdinetError::JsonParse(_)));
}

#[tokio::test]
async fn download_returns_zip_and_pdf() {
    let server = MockServer::start_async().await;
    let zip = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/documents/S1000001")
                .query_param("type", "1")
                .query_param("Subscription-Key", "test-key");
            then.status(200)
                .header("content-type", "application/octet-stream")
                .body(b"PK\x03\x04zip");
        })
        .await;
    server
        .mock_async(|when, then| {
            when.path("/documents/S1000001").query_param("type", "2");
            then.status(200)
                .header("content-type", "application/pdf")
                .body(b"%PDF-1.7");
        })
        .await;
    server
        .mock_async(|when, then| {
            when.path("/documents/S1000001").query_param("type", "5");
            then.status(200)
                .header("content-type", "Application/Zip")
                .body(b"PK\x03\x04csv");
        })
        .await;

    let api = api(&server);
    let file = api.download("S1000001", DocumentKind::Main).await.unwrap();
    zip.assert_async().await;
    assert_eq!(file.format, FileFormat::Zip);
    assert_eq!(file.bytes, b"PK\x03\x04zip");

    let file = api.download("S1000001", DocumentKind::Pdf).await.unwrap();
    assert_eq!(file.format, FileFormat::Pdf);
    assert_eq!(file.bytes, b"%PDF-1.7");

    let file = api.download("S1000001", DocumentKind::Csv).await.unwrap();
    assert_eq!(file.format, FileFormat::Zip);
}

#[tokio::test]
async fn download_errors() {
    let server = MockServer::start_async().await;
    let json = |then: httpmock::Then, body: &str| {
        then.status(200)
            .header("content-type", "application/json; charset=utf-8")
            .body(body);
    };
    server
        .mock_async(|when, then| {
            when.path("/documents/S0000404");
            json(then, NOT_FOUND);
        })
        .await;
    server
        .mock_async(|when, then| {
            when.path("/documents/S0000429");
            json(
                then,
                r#"{"StatusCode": 429, "message": "Too Many Requests"}"#,
            );
        })
        .await;
    server
        .mock_async(|when, then| {
            when.path("/documents/S0000200");
            json(then, r#"{"unexpected":true}"#);
        })
        .await;
    server
        .mock_async(|when, then| {
            when.path("/documents/S0000503");
            then.status(503)
                .header("content-type", "text/html")
                .body("Sorry");
        })
        .await;
    server
        .mock_async(|when, then| {
            when.path("/documents/S0000999");
            then.status(200)
                .header("content-type", "text/html")
                .body("<html>");
        })
        .await;
    server
        .mock_async(|when, then| {
            when.path("/documents/S0000000");
            then.status(200).body("no content type");
        })
        .await;

    let api = api(&server);
    let download = |id: &'static str| {
        let api = api.clone();
        async move { api.download(id, DocumentKind::Main).await.unwrap_err() }
    };
    assert!(download("S0000404").await.is_not_found());
    assert!(download("S0000429").await.is_rate_limited());
    assert!(matches!(
        download("S0000200").await,
        EdinetError::HttpStatus(200, _)
    ));
    assert!(
        matches!(download("S0000503").await, EdinetError::HttpStatus(503, body) if body == "Sorry")
    );
    assert!(
        matches!(download("S0000999").await, EdinetError::UnexpectedContentType(t) if t == "text/html")
    );
    assert!(
        matches!(download("S0000000").await, EdinetError::UnexpectedContentType(t) if t.is_empty())
    );
}

#[tokio::test]
async fn download_rejects_bad_ids_before_sending() {
    let api = EdinetApi::with_api_key("k").with_base_url("http://127.0.0.1:9");
    for id in ["", "../documents.json", "S100 0001"] {
        let error = api.download(id, DocumentKind::Pdf).await.unwrap_err();
        assert!(matches!(error, EdinetError::InvalidArgument(_)), "{id}");
    }
}

#[tokio::test]
async fn bad_base_url() {
    let api = EdinetApi::with_api_key("k").with_base_url("not a url");
    let error = api.metadata(date("2023-04-03")).await.unwrap_err();
    assert!(matches!(error, EdinetError::InvalidArgument(_)));
    let error = api
        .download("S1000001", DocumentKind::Main)
        .await
        .unwrap_err();
    assert!(matches!(error, EdinetError::InvalidArgument(_)));
}

#[tokio::test]
async fn connection_errors_hide_the_key() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);

    let api = EdinetApi::with_api_key("secret-key")
        .with_client(edinet_api::reqwest::Client::new())
        .with_base_url(format!("http://{address}"));
    let error = api.documents(date("2023-04-03")).await.unwrap_err();
    assert!(matches!(error, EdinetError::Connection(_)));
    assert!(!error.is_timeout());
    assert!(error.source().is_some());
    let text = format!("{error} {error:?}");
    assert!(!text.contains("secret-key"), "{text}");

    let error = api
        .download("S1000001", DocumentKind::Main)
        .await
        .unwrap_err();
    assert!(matches!(error, EdinetError::Connection(_)));
}

/// Server that accepts one connection, reads the request and answers with `response`.
async fn raw_server(response: &'static [u8]) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buffer = [0; 4096];
                let _ = socket.read(&mut buffer).await;
                let _ = socket.write_all(response).await;
                // Keep the connection open so the client has to time out
                tokio::time::sleep(Duration::from_secs(5)).await;
            });
        }
    });
    format!("http://{address}")
}

#[tokio::test]
async fn timeouts() {
    // Headers never arrive
    let base = raw_server(b"").await;
    let api = EdinetApi::with_api_key("k")
        .with_base_url(base)
        .with_timeout(Duration::from_millis(200));
    let error = api.metadata(date("2023-04-03")).await.unwrap_err();
    assert!(error.is_timeout(), "{error}");
}

#[tokio::test]
async fn truncated_bodies() {
    const TRUNCATED: &[u8] =
        b"HTTP/1.1 200 OK\r\ncontent-type: application/octet-stream\r\ncontent-length: 100\r\n\r\nPK";
    let base = raw_server(TRUNCATED).await;
    let api = EdinetApi::with_api_key("k")
        .with_base_url(base)
        .with_timeout(Duration::from_millis(300));
    let error = api.documents(date("2023-04-03")).await.unwrap_err();
    assert!(matches!(error, EdinetError::Connection(_)), "{error}");
    let error = api
        .download("S1000001", DocumentKind::Main)
        .await
        .unwrap_err();
    assert!(matches!(error, EdinetError::Connection(_)), "{error}");
}
