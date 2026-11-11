//! Responses of the document list API.

use crate::code::{
    DisclosureStatus, DocInfoEditStatus, DocTypeCode, LegalStatus, OrdinanceCode, WithdrawalStatus,
};
use crate::{Date, DateTime};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Summary of the document list for one date.
///
/// Returned by [`EdinetApi::metadata`](crate::EdinetApi::metadata) and part
/// of every [`DocumentList`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Metadata {
    /// Name of the API, `提出された書類を把握するためのAPI`.
    pub title: String,
    /// Parameters of the request.
    pub parameter: Parameter,
    /// Number of entries.
    pub resultset: ResultSet,
    /// When the list was last rebuilt (JST). It changes even when the list does not.
    pub process_date_time: DateTime,
    /// `200` on success.
    pub status: String,
    /// `OK` on success.
    pub message: String,
}

/// Parameters echoed in [`Metadata`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Parameter {
    /// Requested file date.
    pub date: Date,
    /// `1` for metadata only, `2` for metadata and the list.
    #[serde(rename = "type")]
    pub list_type: String,
}

/// Size of the list in [`Metadata`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ResultSet {
    /// Number of entries for the date.
    pub count: u32,
}

/// Document list for one date, returned by
/// [`EdinetApi::documents`](crate::EdinetApi::documents).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct DocumentList {
    /// Summary of the list.
    pub metadata: Metadata,
    /// Entries, ordered by [`seq_number`](Document::seq_number).
    pub results: Vec<Document>,
}

/// One entry of the document list.
///
/// Besides submitted documents, the list contains entries that record a
/// withdrawal, a correction or a change in disclosure. Those entries reuse the
/// `doc_id` of the affected document; see [`withdrawal_status`](Self::withdrawal_status),
/// [`doc_info_edit_status`](Self::doc_info_edit_status) and
/// [`disclosure_status`](Self::disclosure_status).
///
/// When a document is withdrawn or its inspection period ends, every field
/// except `seq_number` and `doc_id` (and `parent_doc_id` for withdrawals)
/// becomes `None`, and the flags become `false`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub struct Document {
    /// Sequence number within the date. It never changes once assigned, so
    /// entries above the last number you saw are new.
    pub seq_number: u32,
    /// Document ID (書類管理番号), such as `S100ABCD`. Pass it to
    /// [`EdinetApi::download`](crate::EdinetApi::download).
    #[serde(rename = "docID")]
    pub doc_id: String,
    /// Filer's EDINET code, such as `E00001`.
    pub edinet_code: Option<String>,
    /// Filer's securities code (5 digits).
    pub sec_code: Option<String>,
    /// Filer's corporate number (法人番号, 13 digits).
    #[serde(rename = "JCN")]
    pub jcn: Option<String>,
    /// Filer's name.
    pub filer_name: Option<String>,
    /// Fund code.
    pub fund_code: Option<String>,
    /// Cabinet Office Ordinance.
    pub ordinance_code: Option<OrdinanceCode>,
    /// Form code (様式コード). The list of values is in Appendix 1 of the specification.
    pub form_code: Option<String>,
    /// Document type.
    pub doc_type_code: Option<DocTypeCode>,
    /// Start of the fiscal period, for annual, semi-annual and quarterly reports.
    pub period_start: Option<Date>,
    /// End of the fiscal period, for annual, semi-annual and quarterly reports.
    pub period_end: Option<Date>,
    /// When the document was submitted (JST).
    pub submit_date_time: Option<DateTime>,
    /// Description shown in EDINET search results.
    pub doc_description: Option<String>,
    /// EDINET code of the issuer, for large shareholding reports.
    pub issuer_edinet_code: Option<String>,
    /// EDINET code of the target company, for tender offers.
    pub subject_edinet_code: Option<String>,
    /// EDINET codes of subsidiaries, comma separated. See
    /// [`subsidiary_edinet_codes`](Self::subsidiary_edinet_codes).
    pub subsidiary_edinet_code: Option<String>,
    /// Reasons for an extraordinary report, comma separated, such as
    /// `第19条第2項第1号`. See [`current_report_reasons`](Self::current_report_reasons).
    pub current_report_reason: Option<String>,
    /// Document ID of the parent document, for amendments, withdrawals and similar.
    #[serde(rename = "parentDocID")]
    pub parent_doc_id: Option<String>,
    /// When an officer corrected or hid the document, or when a paper or disk
    /// submission was entered (JST).
    pub ope_date_time: Option<DateTime>,
    /// Withdrawal status.
    pub withdrawal_status: WithdrawalStatus,
    /// Correction status.
    pub doc_info_edit_status: DocInfoEditStatus,
    /// Disclosure status.
    pub disclosure_status: DisclosureStatus,
    /// Has XBRL ([`DocumentKind::Main`](crate::DocumentKind::Main) then contains it).
    #[serde(with = "flag")]
    pub xbrl_flag: bool,
    /// Has a PDF ([`DocumentKind::Pdf`](crate::DocumentKind::Pdf)).
    #[serde(with = "flag")]
    pub pdf_flag: bool,
    /// Has alternative documents or attachments
    /// ([`DocumentKind::Attachment`](crate::DocumentKind::Attachment)).
    #[serde(with = "flag")]
    pub attach_doc_flag: bool,
    /// Has English files ([`DocumentKind::English`](crate::DocumentKind::English)).
    #[serde(with = "flag")]
    pub english_doc_flag: bool,
    /// Has CSV converted from XBRL ([`DocumentKind::Csv`](crate::DocumentKind::Csv)).
    #[serde(with = "flag")]
    pub csv_flag: bool,
    /// Public inspection status.
    pub legal_status: LegalStatus,
}

impl Document {
    /// [`subsidiary_edinet_code`](Self::subsidiary_edinet_code) split at commas.
    pub fn subsidiary_edinet_codes(&self) -> Vec<&str> {
        split(&self.subsidiary_edinet_code)
    }

    /// [`current_report_reason`](Self::current_report_reason) split at commas.
    pub fn current_report_reasons(&self) -> Vec<&str> {
        split(&self.current_report_reason)
    }
}

fn split(value: &Option<String>) -> Vec<&str> {
    value
        .as_deref()
        .into_iter()
        .flat_map(|s| s.split(','))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// `"1"` / `"0"` flags as `bool`.
mod flag {
    use super::*;

    pub fn serialize<S: Serializer>(value: &bool, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(if *value { "1" } else { "0" })
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
        match Option::<String>::deserialize(deserializer)?.as_deref() {
            Some("1") => Ok(true),
            Some("0") | None => Ok(false),
            Some(other) => Err(D::Error::custom(format!(
                "expected \"0\" or \"1\": {other:?}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn entry(overrides: Value) -> Value {
        let mut value = json!({
            "seqNumber": 1, "docID": "S1000001", "edinetCode": null, "secCode": null,
            "JCN": null, "filerName": null, "fundCode": null, "ordinanceCode": null,
            "formCode": null, "docTypeCode": null, "periodStart": null, "periodEnd": null,
            "submitDateTime": null, "docDescription": null, "issuerEdinetCode": null,
            "subjectEdinetCode": null, "subsidiaryEdinetCode": null,
            "currentReportReason": null, "parentDocID": null, "opeDateTime": null,
            "withdrawalStatus": "0", "docInfoEditStatus": "0", "disclosureStatus": "0",
            "xbrlFlag": "0", "pdfFlag": "0", "attachDocFlag": "0", "englishDocFlag": "0",
            "csvFlag": "0", "legalStatus": "0"
        });
        for (key, v) in overrides.as_object().unwrap() {
            value[key] = v.clone();
        }
        value
    }

    #[test]
    fn flags() {
        let doc: Document = serde_json::from_value(entry(json!({"xbrlFlag": "1"}))).unwrap();
        assert!(doc.xbrl_flag);
        assert!(!doc.pdf_flag);
        let doc: Document = serde_json::from_value(entry(json!({"csvFlag": null}))).unwrap();
        assert!(!doc.csv_flag);
        assert!(serde_json::from_value::<Document>(entry(json!({"pdfFlag": 1}))).is_err());
        let error = serde_json::from_value::<Document>(entry(json!({"pdfFlag": "Y"})));
        assert!(
            error
                .unwrap_err()
                .to_string()
                .contains("expected \"0\" or \"1\"")
        );

        let back = serde_json::to_value(
            serde_json::from_value::<Document>(entry(json!({"xbrlFlag": "1"}))).unwrap(),
        )
        .unwrap();
        assert_eq!(back["xbrlFlag"], "1");
        assert_eq!(back["pdfFlag"], "0");
    }

    #[test]
    fn splits_comma_lists() {
        let doc: Document = serde_json::from_value(entry(json!({
            "subsidiaryEdinetCode": "E00001,E00002, E00003",
            "currentReportReason": "第19条第2項第1号,第19条第2項第4号"
        })))
        .unwrap();
        assert_eq!(
            doc.subsidiary_edinet_codes(),
            ["E00001", "E00002", "E00003"]
        );
        assert_eq!(
            doc.current_report_reasons(),
            ["第19条第2項第1号", "第19条第2項第4号"]
        );

        let doc: Document = serde_json::from_value(entry(json!({}))).unwrap();
        assert!(doc.subsidiary_edinet_codes().is_empty());
        let doc: Document =
            serde_json::from_value(entry(json!({"subsidiaryEdinetCode": ""}))).unwrap();
        assert!(doc.subsidiary_edinet_codes().is_empty());
    }
}
