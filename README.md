# edinet-api

[![Crates.io](https://img.shields.io/crates/v/edinet-api?style=flat-square)](https://crates.io/crates/edinet-api)
[![Documentation](https://img.shields.io/docsrs/edinet-api?style=flat-square)](https://docs.rs/edinet-api)
[![CI](https://img.shields.io/github/actions/workflow/status/uiuifree/rust-edinet-api/ci.yml?branch=main&style=flat-square&label=CI)](https://github.com/uiuifree/rust-edinet-api/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue?style=flat-square)](https://github.com/uiuifree/rust-edinet-api/blob/main/Cargo.toml)
[![License](https://img.shields.io/crates/l/edinet-api?style=flat-square)](https://github.com/uiuifree/rust-edinet-api/blob/main/LICENSE)

Unofficial async Rust client for the [EDINET API](https://disclosure2dl.edinet-fsa.go.jp/guide/static/disclosure/WZEK0110.html)
v2, the API of EDINET, the electronic disclosure system of Japan's Financial Services Agency (FSA).
It lists the documents filed on a given date and downloads them as ZIP (XBRL, CSV, attachments,
English files) or PDF.

This crate is not made or endorsed by the FSA.

## Setup

Create an account on EDINET and issue an API key, as described in chapter 2 of the
[specification](https://disclosure2dl.edinet-fsa.go.jp/guide/static/disclosure/download/ESE140206.pdf).
The key is not bundled with this crate; pass your own.

```toml
[dependencies]
edinet-api = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Usage

```rust,no_run
use edinet_api::code::DocTypeCode;
use edinet_api::{DocumentKind, EdinetApi};

#[tokio::main]
async fn main() -> Result<(), edinet_api::EdinetError> {
    let api = EdinetApi::with_api_key(std::env::var("EDINET_API_KEY").unwrap());

    let list = api.documents("2026-06-30".parse()?).await?;
    println!("{} entries", list.metadata.resultset.count);

    for doc in &list.results {
        if doc.doc_type_code == Some(DocTypeCode::AnnualSecuritiesReport) && doc.xbrl_flag {
            println!("{} {:?}", doc.doc_id, doc.filer_name);
            let file = api.download(&doc.doc_id, DocumentKind::Main).await?;
            std::fs::write(format!("{}.{}", doc.doc_id, file.format.extension()), &file.bytes).unwrap();
        }
    }
    Ok(())
}
```

Requests time out after 60 seconds. Change it with `with_timeout`, or pass your own
`reqwest::Client` with `with_client`.

### Document list

`documents(date)` returns every entry for one file date. `metadata(date)` returns only the
summary, which is enough to see whether the count changed since the last call. Today's list is
updated about every minute from 8:30 JST, and `seq_number` never changes once assigned, so entries
with a number above the last one you saw are new.

The list also holds entries that record a withdrawal, a correction by a Local Finance Bureau
officer, or a change in disclosure. They reuse the `doc_id` of the affected document and are told
apart by `withdrawal_status`, `doc_info_edit_status` and `disclosure_status`. Withdrawn documents
and documents past their inspection period keep only `seq_number` and `doc_id`.

Codes are typed: `DocTypeCode`, `OrdinanceCode` and the status enums in the `code` module, with
`name()` returning the official Japanese name. Unknown codes are kept in `Other(String)`. Dates are
`Date` (`YYYY-MM-DD`) and `DateTime` (`YYYY-MM-DD hh:mm`, JST).

### Downloads

| `DocumentKind` | `type` | Content | Flag |
|----------------|--------|---------|------|
| `Main` | `1` | main document, audit report, XBRL | `xbrl_flag` for XBRL |
| `Pdf` | `2` | PDF | `pdf_flag` |
| `Attachment` | `3` | alternative documents and attachments | `attach_doc_flag` |
| `English` | `4` | English files | `english_doc_flag` |
| `Csv` | `5` | XBRL converted to CSV | `csv_flag` |

All are ZIP except `Pdf`. The crate returns the bytes and does not unzip them.

### Errors

EDINET answers most errors with HTTP status `200` and a JSON body. The crate reads that body and
returns `EdinetError::Api { status, message }`. For downloads it checks `Content-Type` as the
specification recommends.

```rust,no_run
use edinet_api::{EdinetApi, EdinetError};

# async fn run(api: EdinetApi) {
match api.documents("2026-06-30".parse().unwrap()).await {
    Ok(list) => println!("{}", list.results.len()),
    Err(e) if e.is_unauthorized() => eprintln!("check the API key"),
    Err(e) if e.is_rate_limited() => eprintln!("too many requests, wait and retry"),
    Err(e) if e.is_not_found() => eprintln!("no list for this date"),
    Err(e) => eprintln!("{e}"),
}
# }
```

| Status | Meaning |
|--------|---------|
| `400` | invalid parameter |
| `401` | missing or invalid API key |
| `404` | date out of range, or document not available |
| `429` | too many requests |
| `500` | server error |

The API key is sent in the URL, as the specification requires. The crate removes the URL from
connection errors and hides the key in `Debug` output.

## Command line tool

The `cli` feature adds an `edinet` command. It reads the key from `EDINET_API_KEY` (or
`--api-key`) and prints JSON to stdout.

```bash
cargo install edinet-api --features cli

edinet metadata 2026-06-30
edinet list 2026-06-30 --doc-type 120 --doc-type 140
edinet list 2026-06-30 --sec-code 7203 --ndjson
edinet download S100ABCD --kind pdf -o report.pdf
edinet codes
```

`list` filters by `--doc-type`, `--edinet-code`, `--sec-code` (prefix match, so a 4-digit code
works) and `--after-seq N`, which keeps only entries added after the last run; `meta.last_seq`
holds the number to pass next time. `--ndjson` prints one document per line. `download` does not
overwrite an existing file unless `--force` is given.

Errors are printed to stderr as `{"ok": false, "error": {"kind", "message", "status"}}`.

| Exit code | Meaning |
|-----------|---------|
| `0` | success |
| `1` | other failure, such as a file that cannot be written |
| `2` | invalid arguments |
| `3` | missing or invalid API key |
| `4` | API or network error |
| `5` | rate limited |
| `6` | not found |

## Terms of use

Follow the [EDINET terms of use](https://disclosure2dl.edinet-fsa.go.jp/guide/static/disclosure/WZEK0030.html).
In particular:

- Do not send many requests in a short time. This crate does not throttle or retry; leave enough
  time between requests.
- Content from EDINET is available under the Public Data License (PDL1.0). Cite the source when you
  use it, and state that you edited it if you did.

## Source

The code names and field descriptions in this crate are based on the EDINET API specification.

出典：[EDINET API 仕様書（Version 2）](https://disclosure2dl.edinet-fsa.go.jp/guide/static/disclosure/download/ESE140206.pdf)
（金融庁、2026 年 6 月）、[PDL1.0](https://www.digital.go.jp/resources/open_data/public_data_license_v1.0)
をもとに uiuifree 作成

## Development

```bash
cargo test
EDINET_API_KEY=... cargo test --test live -- --ignored --test-threads 1
EDINET_API_KEY=... cargo run --example list -- 2026-06-30
cargo run --features cli -- list 2026-06-30
```

See [CONTRIBUTING.md](https://github.com/uiuifree/rust-edinet-api/blob/main/CONTRIBUTING.md).

## License

MIT
