//! Runs the `edinet` binary against a mock server.

use httpmock::prelude::*;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const METADATA: &str = include_str!("fixtures/metadata.json");
const DOCUMENTS: &str = include_str!("fixtures/documents.json");

fn edinet(server: Option<&MockServer>, dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_edinet"));
    command
        .args(args)
        .current_dir(dir)
        .env("EDINET_API_KEY", "test-key")
        .env_remove("EDINET_BASE_URL");
    if let Some(server) = server {
        command.env("EDINET_BASE_URL", server.base_url());
    }
    command.output().unwrap()
}

fn stdout(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// Exit code and parsed stderr of a failed run.
fn failure(output: &Output) -> (i32, Value) {
    let stderr = serde_json::from_slice(&output.stderr)
        .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&output.stderr)));
    (output.status.code().unwrap(), stderr)
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("edinet-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn list_server() -> MockServer {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.path("/documents.json")
            .query_param("type", "2")
            .query_param("Subscription-Key", "test-key");
        then.status(200).body(DOCUMENTS);
    });
    server.mock(|when, then| {
        when.path("/documents.json").query_param("type", "1");
        then.status(200).body(METADATA);
    });
    server
}

#[test]
fn metadata() {
    let server = list_server();
    let dir = temp_dir("metadata");
    let out = stdout(&edinet(Some(&server), &dir, &["metadata", "2023-04-03"]));
    assert_eq!(out["ok"], true);
    assert_eq!(out["command"], "metadata");
    assert_eq!(out["params"]["date"], "2023-04-03");
    assert_eq!(out["data"]["metadata"]["resultset"]["count"], 1);

    // Without rows, --ndjson prints the whole result on one line
    let output = edinet(Some(&server), &dir, &["metadata", "2023-04-03", "--ndjson"]);
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.lines().count(), 1);
}

#[test]
fn list_with_filters() {
    let server = list_server();
    let dir = temp_dir("list");
    let out = stdout(&edinet(Some(&server), &dir, &["list", "2023-06-13"]));
    assert_eq!(out["rows"].as_array().unwrap().len(), 3);
    assert_eq!(out["meta"]["total"], 3);
    assert_eq!(out["meta"]["last_seq"], 3);
    assert_eq!(out["meta"]["process_date_time"], "2023-06-14 00:31");

    let out = stdout(&edinet(
        Some(&server),
        &dir,
        &[
            "list",
            "2023-06-13",
            "--doc-type",
            "180",
            "--doc-type",
            "120",
        ],
    ));
    assert_eq!(out["meta"]["returned"], 2);

    let out = stdout(&edinet(
        Some(&server),
        &dir,
        &["list", "2023-06-13", "--sec-code", "2000"],
    ));
    assert_eq!(out["rows"][0]["docID"], "S1000003");

    let out = stdout(&edinet(
        Some(&server),
        &dir,
        &[
            "list",
            "2023-06-13",
            "--edinet-code",
            "E10001",
            "--after-seq",
            "0",
        ],
    ));
    assert_eq!(out["meta"]["returned"], 1);

    let output = edinet(
        Some(&server),
        &dir,
        &["list", "2023-06-13", "--after-seq", "1", "--ndjson"],
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let ids: Vec<String> = text
        .lines()
        .map(|line| {
            serde_json::from_str::<Value>(line).unwrap()["docID"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(ids, ["S1000002", "S1000003"]);
}

#[test]
fn download() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.path("/documents/S1000001").query_param("type", "2");
        then.status(200)
            .header("content-type", "application/pdf")
            .body(b"%PDF-1.7");
    });
    server.mock(|when, then| {
        when.path("/documents/S1000001").query_param_exists("type");
        then.status(200)
            .header("content-type", "application/octet-stream")
            .body(b"PK");
    });
    let dir = temp_dir("download");

    let out = stdout(&edinet(
        Some(&server),
        &dir,
        &["download", "S1000001", "--kind", "pdf"],
    ));
    assert_eq!(out["data"]["path"], "S1000001.pdf");
    assert_eq!(out["data"]["format"], "pdf");
    assert_eq!(out["data"]["bytes"], 8);
    assert_eq!(out["params"]["kind"], "2");
    assert_eq!(
        std::fs::read(dir.join("S1000001.pdf")).unwrap(),
        b"%PDF-1.7"
    );

    let out = stdout(&edinet(Some(&server), &dir, &["download", "S1000001"]));
    assert_eq!(out["data"]["path"], "S1000001.zip");

    // Existing files are kept unless --force is given; no request is sent
    let (code, error) = failure(&edinet(None, &dir, &["download", "S1000001"]));
    assert_eq!(code, 2);
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--force")
    );
    stdout(&edinet(
        Some(&server),
        &dir,
        &["download", "S1000001", "--force"],
    ));

    for (kind, code) in [("attachment", "3"), ("english", "4"), ("csv", "5")] {
        let file = format!("{kind}.zip");
        let out = stdout(&edinet(
            Some(&server),
            &dir,
            &["download", "S1000001", "--kind", kind, "-o", &file],
        ));
        assert_eq!(out["params"]["kind"], code);
    }

    let out = stdout(&edinet(
        Some(&server),
        &dir,
        &["download", "S1000001", "-o", "main.zip"],
    ));
    assert_eq!(out["data"]["path"], "main.zip");

    let (code, error) = failure(&edinet(
        Some(&server),
        &dir,
        &["download", "S1000001", "-o", "missing/main.zip"],
    ));
    assert_eq!(
        (code, error["error"]["kind"].as_str()),
        (1, Some("internal"))
    );
}

#[test]
fn api_errors_set_the_exit_code() {
    let server = MockServer::start();
    let body = |date: &str, status: u16, body: &'static str| {
        server.mock(|when, then| {
            when.query_param("date", date);
            then.status(status).body(body);
        });
    };
    body(
        "2023-01-01",
        200,
        r#"{"StatusCode": 401, "message": "Access denied"}"#,
    );
    body(
        "2023-01-02",
        200,
        r#"{"StatusCode": 429, "message": "Too Many Requests"}"#,
    );
    body(
        "2023-01-03",
        200,
        r#"{"metadata":{"status":"404","message":"Not Found"}}"#,
    );
    body("2023-01-04", 503, "Sorry");
    let dir = temp_dir("errors");

    for (date, code, kind) in [
        ("2023-01-01", 3, "auth"),
        ("2023-01-02", 5, "rate_limit"),
        ("2023-01-03", 6, "not_found"),
        ("2023-01-04", 4, "api"),
    ] {
        let (exit, error) = failure(&edinet(Some(&server), &dir, &["list", date]));
        assert_eq!(
            (exit, error["error"]["kind"].as_str()),
            (code, Some(kind)),
            "{date}"
        );
    }
    let (code, _) = failure(&edinet(Some(&server), &dir, &["metadata", "2023-01-03"]));
    assert_eq!(code, 6);
    let (_, error) = failure(&edinet(Some(&server), &dir, &["list", "2023-01-03"]));
    assert_eq!(error["ok"], false);
    assert_eq!(error["error"]["status"], 404);

    let (code, _) = failure(&edinet(Some(&server), &dir, &["download", "S1/../x"]));
    assert_eq!(code, 2);
}

#[test]
fn usage_errors() {
    let dir = temp_dir("usage");
    let (code, error) = failure(&edinet(None, &dir, &["list", "2023-02-30"]));
    assert_eq!(code, 2);
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("2023-02-30")
    );

    for args in [
        ["metadata", "2023-04-03"],
        ["list", "2023-04-03"],
        ["download", "S1000001"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_edinet"))
            .args(args)
            .current_dir(&dir)
            .env_remove("EDINET_API_KEY")
            .output()
            .unwrap();
        let (code, error) = failure(&output);
        assert_eq!((code, error["error"]["kind"].as_str()), (3, Some("auth")));
    }

    let help = edinet(None, &dir, &["--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("EDINET"));
    assert!(edinet(None, &dir, &["--version"]).status.success());
}

#[test]
fn codes() {
    let dir = temp_dir("codes");
    let out = stdout(&edinet(None, &dir, &["codes"]));
    let doc_types = out["data"]["doc_type"].as_array().unwrap();
    assert!(
        doc_types
            .iter()
            .any(|c| c["code"] == "120" && c["name"] == "有価証券報告書")
    );
    assert_eq!(out["data"]["ordinance"].as_array().unwrap().len(), 7);
}
