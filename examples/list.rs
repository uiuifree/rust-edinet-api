//! Lists the documents filed on a date and downloads the first PDF.
//!
//! ```bash
//! EDINET_API_KEY=... cargo run --example list -- 2026-06-30
//! ```

use edinet_api::{DocumentKind, EdinetApi};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let date = std::env::args()
        .nth(1)
        .ok_or("usage: list YYYY-MM-DD")?
        .parse()?;
    let api = EdinetApi::with_api_key(std::env::var("EDINET_API_KEY")?);

    let list = api.documents(date).await?;
    println!(
        "{} entries, updated {}",
        list.metadata.resultset.count, list.metadata.process_date_time
    );
    for doc in &list.results {
        let doc_type = doc
            .doc_type_code
            .as_ref()
            .and_then(|code| code.name())
            .unwrap_or("-");
        println!(
            "{:>5} {} {} {}",
            doc.seq_number,
            doc.doc_id,
            doc_type,
            doc.filer_name.as_deref().unwrap_or("-")
        );
    }

    if let Some(doc) = list.results.iter().find(|doc| doc.pdf_flag == Some(true)) {
        let file = api.download(&doc.doc_id, DocumentKind::Pdf).await?;
        let path = format!("{}.{}", doc.doc_id, file.format.extension());
        std::fs::write(&path, &file.bytes)?;
        println!("saved {path}");
    }
    Ok(())
}
