//! `edinet`: command line tool for the EDINET API.
//!
//! Prints JSON to stdout. Errors go to stderr as JSON, with the exit code
//! telling the kind of failure.

use clap::error::ErrorKind as ClapErrorKind;
use clap::{Parser, Subcommand, ValueEnum};
use edinet_api::code::{DocTypeCode, OrdinanceCode};
use edinet_api::{Date, Document, DocumentKind, EdinetApi, EdinetError};
use serde_json::{Value, json};
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

/// Command line tool for the EDINET API v2 (unofficial).
///
/// Output is JSON on stdout. Errors are JSON on stderr with exit code
/// 2 (usage), 3 (API key), 4 (API or network), 5 (rate limit), 6 (not found)
/// or 1 (other). Keep the request rate low, as the EDINET terms of use require.
#[derive(Debug, Parser)]
#[command(name = "edinet", version)]
struct Cli {
    /// EDINET API key.
    #[arg(long, env = "EDINET_API_KEY", hide_env_values = true, global = true)]
    api_key: Option<String>,

    /// API base URL, for tests.
    #[arg(long, env = "EDINET_BASE_URL", hide = true, global = true)]
    base_url: Option<String>,

    /// Print one JSON object per line instead of one pretty JSON document.
    #[arg(long, global = true)]
    ndjson: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Summary of the document list for a date (count and update time).
    Metadata {
        /// File date, YYYY-MM-DD.
        date: Date,
    },
    /// Documents filed on a date.
    List {
        /// File date, YYYY-MM-DD.
        date: Date,
        /// Only these document types, such as 120 (annual securities report). Repeatable.
        #[arg(long = "doc-type", value_name = "CODE")]
        doc_types: Vec<String>,
        /// Only this filer EDINET code, such as E00001. Repeatable.
        #[arg(long = "edinet-code", value_name = "CODE")]
        edinet_codes: Vec<String>,
        /// Only this securities code (4 or 5 digits). Repeatable.
        #[arg(long = "sec-code", value_name = "CODE")]
        sec_codes: Vec<String>,
        /// Only entries with a sequence number above this, to get what is new since the last run.
        #[arg(long, value_name = "N")]
        after_seq: Option<u32>,
    },
    /// Download a document as ZIP or PDF.
    Download {
        /// Document ID, such as S100ABCD.
        doc_id: String,
        /// Which files to download.
        #[arg(long, value_enum, default_value_t = Kind::Main)]
        kind: Kind,
        /// Output path. Defaults to DOC_ID.zip or DOC_ID.pdf in the current directory.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Overwrite the output file if it exists.
        #[arg(long)]
        force: bool,
    },
    /// Known document type and ordinance codes with their Japanese names.
    Codes,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Kind {
    /// Main document, audit report and XBRL (ZIP).
    Main,
    /// PDF.
    Pdf,
    /// Alternative documents and attachments (ZIP).
    Attachment,
    /// English files (ZIP).
    English,
    /// XBRL converted to CSV (ZIP).
    Csv,
}

impl From<Kind> for DocumentKind {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Main => DocumentKind::Main,
            Kind::Pdf => DocumentKind::Pdf,
            Kind::Attachment => DocumentKind::Attachment,
            Kind::English => DocumentKind::English,
            Kind::Csv => DocumentKind::Csv,
        }
    }
}

/// Failure category, which decides the exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Failure {
    Internal = 1,
    Usage = 2,
    Auth = 3,
    Api = 4,
    RateLimit = 5,
    NotFound = 6,
}

impl Failure {
    fn name(self) -> &'static str {
        match self {
            Failure::Internal => "internal",
            Failure::Usage => "usage",
            Failure::Auth => "auth",
            Failure::Api => "api",
            Failure::RateLimit => "rate_limit",
            Failure::NotFound => "not_found",
        }
    }
}

struct CliError {
    failure: Failure,
    message: String,
    status: Option<u16>,
}

impl CliError {
    fn new(failure: Failure, message: impl Into<String>) -> Self {
        CliError {
            failure,
            message: message.into(),
            status: None,
        }
    }

    fn to_json(&self) -> Value {
        let mut error = json!({"kind": self.failure.name(), "message": self.message});
        if let Some(status) = self.status {
            error["status"] = status.into();
        }
        json!({"ok": false, "error": error})
    }
}

impl From<EdinetError> for CliError {
    fn from(error: EdinetError) -> Self {
        let failure = match &error {
            e if e.is_unauthorized() => Failure::Auth,
            e if e.is_rate_limited() => Failure::RateLimit,
            e if e.is_not_found() => Failure::NotFound,
            EdinetError::InvalidArgument(_) => Failure::Usage,
            _ => Failure::Api,
        };
        CliError {
            failure,
            message: error.to_string(),
            status: error.status(),
        }
    }
}

/// Successful result: `rows` are printed one per line with `--ndjson`.
struct Output {
    command: &'static str,
    params: Value,
    rows: Option<Vec<Value>>,
    data: Option<Value>,
    meta: Value,
}

impl Output {
    fn to_json(&self) -> Value {
        let mut out = json!({"ok": true, "command": self.command, "params": self.params});
        if let Some(rows) = &self.rows {
            out["rows"] = Value::Array(rows.clone());
        }
        if let Some(data) = &self.data {
            out["data"] = data.clone();
        }
        out["meta"] = self.meta.clone();
        out
    }

    fn write(&self, w: &mut impl Write, ndjson: bool) -> std::io::Result<()> {
        match (&self.rows, ndjson) {
            (Some(rows), true) => rows.iter().try_for_each(|row| writeln!(w, "{row}")),
            (None, true) => writeln!(w, "{}", self.to_json()),
            (_, false) => writeln!(w, "{:#}", self.to_json()),
        }
    }
}

fn api(cli: &Cli) -> Result<EdinetApi, CliError> {
    let key = cli
        .api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .ok_or_else(|| {
            CliError::new(
                Failure::Auth,
                "no API key: set EDINET_API_KEY or pass --api-key",
            )
        })?;
    let api = EdinetApi::with_api_key(key);
    Ok(match &cli.base_url {
        Some(base_url) => api.with_base_url(base_url),
        None => api,
    })
}

fn to_value(value: impl serde::Serialize) -> Value {
    serde_json::to_value(value).expect("response types serialize to JSON")
}

async fn run(cli: &Cli) -> Result<Output, CliError> {
    match &cli.command {
        Command::Metadata { date } => {
            let metadata = api(cli)?.metadata(*date).await?;
            Ok(Output {
                command: "metadata",
                params: json!({"date": date}),
                rows: None,
                data: Some(to_value(&metadata)),
                meta: json!({}),
            })
        }
        Command::List {
            date,
            doc_types,
            edinet_codes,
            sec_codes,
            after_seq,
        } => {
            let list = api(cli)?.documents(*date).await?;
            let keep = |doc: &Document| {
                let matches = |filter: &[String], value: Option<&str>| {
                    filter.is_empty()
                        || value.is_some_and(|v| filter.iter().any(|f| v.starts_with(f.as_str())))
                };
                after_seq.is_none_or(|n| doc.seq_number > n)
                    && matches(doc_types, doc.doc_type_code.as_ref().map(DocTypeCode::code))
                    && matches(edinet_codes, doc.edinet_code.as_deref())
                    && matches(sec_codes, doc.sec_code.as_deref())
            };
            let rows: Vec<Value> = list
                .results
                .iter()
                .filter(|d| keep(d))
                .map(to_value)
                .collect();
            Ok(Output {
                command: "list",
                params: json!({
                    "date": date,
                    "doc_type": doc_types,
                    "edinet_code": edinet_codes,
                    "sec_code": sec_codes,
                    "after_seq": after_seq,
                }),
                meta: json!({
                    "total": list.metadata.resultset.count,
                    "returned": rows.len(),
                    "process_date_time": list.metadata.process_date_time,
                    "last_seq": list.results.iter().map(|d| d.seq_number).max(),
                }),
                rows: Some(rows),
                data: None,
            })
        }
        Command::Download {
            doc_id,
            kind,
            output,
            force,
        } => {
            let api = api(cli)?;
            let kind = DocumentKind::from(*kind);
            let path = output.clone().unwrap_or_else(|| {
                let extension = if kind == DocumentKind::Pdf {
                    "pdf"
                } else {
                    "zip"
                };
                PathBuf::from(format!("{doc_id}.{extension}"))
            });
            // Check before downloading so a mistake does not cost a request
            if !force && path.exists() {
                return Err(CliError::new(
                    Failure::Usage,
                    format!("{} exists; pass --force to overwrite", path.display()),
                ));
            }
            let file = api.download(doc_id, kind).await?;
            std::fs::write(&path, &file.bytes).map_err(|e| {
                CliError::new(Failure::Internal, format!("{}: {e}", path.display()))
            })?;
            Ok(Output {
                command: "download",
                params: json!({"doc_id": doc_id, "kind": kind.code()}),
                rows: None,
                data: Some(json!({
                    "path": path,
                    "format": file.format.extension(),
                    "bytes": file.bytes.len(),
                })),
                meta: json!({}),
            })
        }
        Command::Codes => {
            let entry = |code: &str, name: Option<&str>| json!({"code": code, "name": name});
            Ok(Output {
                command: "codes",
                params: json!({}),
                rows: None,
                data: Some(json!({
                    "doc_type": DocTypeCode::ALL.iter().map(|c| entry(c.code(), c.name())).collect::<Vec<_>>(),
                    "ordinance": OrdinanceCode::ALL.iter().map(|c| entry(c.code(), c.name())).collect::<Vec<_>>(),
                })),
                meta: json!({}),
            })
        }
    }
}

fn fail(error: CliError) -> ExitCode {
    eprintln!("{}", error.to_json());
    ExitCode::from(error.failure as u8)
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e)
            if matches!(
                e.kind(),
                ClapErrorKind::DisplayHelp | ClapErrorKind::DisplayVersion
            ) =>
        {
            print!("{e}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            let message = e.render().to_string();
            let first = message.lines().next().unwrap_or_default();
            return fail(CliError::new(
                Failure::Usage,
                first.trim_start_matches("error: ").to_string(),
            ));
        }
    };
    match run(&cli).await {
        Ok(output) => print(&output, &mut std::io::stdout().lock(), cli.ndjson),
        Err(error) => fail(error),
    }
}

fn print(output: &Output, w: &mut impl Write, ndjson: bool) -> ExitCode {
    match output.write(w, ndjson) {
        // `edinet list ... | head` closes the pipe early; that is not a failure
        Err(e) if e.kind() != std::io::ErrorKind::BrokenPipe => {
            fail(CliError::new(Failure::Internal, e.to_string()))
        }
        _ => ExitCode::SUCCESS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Failing(std::io::ErrorKind);

    impl Write for Failing {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(self.0.into())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn stdout_errors() {
        let output = Output {
            command: "codes",
            params: json!({}),
            rows: None,
            data: None,
            meta: json!({}),
        };
        let code = print(&output, &mut Failing(std::io::ErrorKind::BrokenPipe), false);
        assert_eq!(code, ExitCode::SUCCESS);
        let code = print(
            &output,
            &mut Failing(std::io::ErrorKind::StorageFull),
            false,
        );
        assert_eq!(code, ExitCode::from(1));
        Failing(std::io::ErrorKind::Other).flush().unwrap();
    }
}
