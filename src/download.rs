/// Which files to download (`type` of the document API, 必要書類).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DocumentKind {
    /// `1`: main document and audit report, with XBRL when
    /// [`xbrl_flag`](crate::Document::xbrl_flag) is `Some(true)`. ZIP.
    Main,
    /// `2`: the PDF shown in EDINET search results. PDF.
    Pdf,
    /// `3`: alternative documents and attachments. ZIP.
    Attachment,
    /// `4`: English files. ZIP.
    English,
    /// `5`: XBRL converted to CSV. ZIP.
    Csv,
}

impl DocumentKind {
    /// Value of the `type` parameter, `1` to `5`.
    pub fn code(&self) -> &'static str {
        match self {
            DocumentKind::Main => "1",
            DocumentKind::Pdf => "2",
            DocumentKind::Attachment => "3",
            DocumentKind::English => "4",
            DocumentKind::Csv => "5",
        }
    }
}

/// Format of a [`Download`], taken from the `Content-Type` header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FileFormat {
    /// ZIP archive.
    Zip,
    /// PDF document.
    Pdf,
}

impl FileFormat {
    /// File extension without the dot: `zip` or `pdf`.
    pub fn extension(&self) -> &'static str {
        match self {
            FileFormat::Zip => "zip",
            FileFormat::Pdf => "pdf",
        }
    }
}

/// File returned by [`EdinetApi::download`](crate::EdinetApi::download).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Download {
    /// ZIP or PDF.
    pub format: FileFormat,
    /// Body of the response.
    pub bytes: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_extensions() {
        let codes: Vec<_> = [
            DocumentKind::Main,
            DocumentKind::Pdf,
            DocumentKind::Attachment,
            DocumentKind::English,
            DocumentKind::Csv,
        ]
        .iter()
        .map(DocumentKind::code)
        .collect();
        assert_eq!(codes, ["1", "2", "3", "4", "5"]);
        assert_eq!(FileFormat::Zip.extension(), "zip");
        assert_eq!(FileFormat::Pdf.extension(), "pdf");
    }
}
