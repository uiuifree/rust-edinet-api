//! Codes used in the document list.
//!
//! Every type keeps codes this crate does not know in an `Other` variant, so
//! a new code from EDINET does not break parsing.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Display, Formatter};

macro_rules! code_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $($(#[$variant_meta:meta])* $variant:ident = $code:literal,)*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum $name {
            $($(#[$variant_meta])* $variant,)*
            /// A code this crate does not know.
            Other(String),
        }

        impl $name {
            /// Every known code, in the order of the specification.
            pub const ALL: &'static [$name] = &[$($name::$variant,)*];

            /// Code as sent by EDINET, such as `120`.
            pub fn code(&self) -> &str {
                match self {
                    $($name::$variant => $code,)*
                    $name::Other(code) => code,
                }
            }
        }

        impl From<&str> for $name {
            fn from(code: &str) -> Self {
                match code {
                    $($code => $name::$variant,)*
                    other => $name::Other(other.to_string()),
                }
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.code())
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.code())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Ok($name::from(String::deserialize(deserializer)?.as_str()))
            }
        }
    };
}

macro_rules! named_code_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $($variant:ident = $code:literal => $label:literal,)*
        }
    ) => {
        code_enum! {
            $(#[$meta])*
            $name {
                $(#[doc = concat!("`", $code, "` ", $label)] $variant = $code,)*
            }
        }

        impl $name {
            /// Official Japanese name from the specification, `None` for
            /// [`Other`](Self::Other).
            pub fn name(&self) -> Option<&'static str> {
                match self {
                    $($name::$variant => Some($label),)*
                    $name::Other(_) => None,
                }
            }
        }
    };
}

named_code_enum! {
    /// Document type (`docTypeCode`, 書類種別コード).
    ///
    /// ```
    /// use edinet_api::code::DocTypeCode;
    ///
    /// let doc_type = DocTypeCode::from("120");
    /// assert_eq!(doc_type, DocTypeCode::AnnualSecuritiesReport);
    /// assert_eq!(doc_type.name(), Some("有価証券報告書"));
    /// ```
    DocTypeCode {
        SecuritiesNotice = "010" => "有価証券通知書",
        ChangeNoticeOfSecuritiesNotice = "020" => "変更通知書（有価証券通知書）",
        SecuritiesRegistrationStatement = "030" => "有価証券届出書",
        AmendedSecuritiesRegistrationStatement = "040" => "訂正有価証券届出書",
        WithdrawalOfRegistration = "050" => "届出の取下げ願い",
        ShelfRegistrationNotice = "060" => "発行登録通知書",
        ChangeNoticeOfShelfRegistrationNotice = "070" => "変更通知書（発行登録通知書）",
        ShelfRegistrationStatement = "080" => "発行登録書",
        AmendedShelfRegistrationStatement = "090" => "訂正発行登録書",
        ShelfRegistrationSupplement = "100" => "発行登録追補書類",
        WithdrawalOfShelfRegistration = "110" => "発行登録取下届出書",
        AnnualSecuritiesReport = "120" => "有価証券報告書",
        AmendedAnnualSecuritiesReport = "130" => "訂正有価証券報告書",
        ConfirmationLetter = "135" => "確認書",
        AmendedConfirmationLetter = "136" => "訂正確認書",
        QuarterlyReport = "140" => "四半期報告書",
        AmendedQuarterlyReport = "150" => "訂正四半期報告書",
        SemiAnnualReport = "160" => "半期報告書",
        AmendedSemiAnnualReport = "170" => "訂正半期報告書",
        ExtraordinaryReport = "180" => "臨時報告書",
        AmendedExtraordinaryReport = "190" => "訂正臨時報告書",
        ParentCompanyStatusReport = "200" => "親会社等状況報告書",
        AmendedParentCompanyStatusReport = "210" => "訂正親会社等状況報告書",
        ShareBuybackStatusReport = "220" => "自己株券買付状況報告書",
        AmendedShareBuybackStatusReport = "230" => "訂正自己株券買付状況報告書",
        InternalControlReport = "235" => "内部統制報告書",
        AmendedInternalControlReport = "236" => "訂正内部統制報告書",
        TenderOfferRegistrationStatement = "240" => "公開買付届出書",
        AmendedTenderOfferRegistrationStatement = "250" => "訂正公開買付届出書",
        TenderOfferWithdrawal = "260" => "公開買付撤回届出書",
        TenderOfferReport = "270" => "公開買付報告書",
        AmendedTenderOfferReport = "280" => "訂正公開買付報告書",
        OpinionReport = "290" => "意見表明報告書",
        AmendedOpinionReport = "300" => "訂正意見表明報告書",
        AnswerToQuestionReport = "310" => "対質問回答報告書",
        AmendedAnswerToQuestionReport = "320" => "訂正対質問回答報告書",
        ExemptionApplication = "330" => "別途買付け禁止の特例を受けるための申出書",
        AmendedExemptionApplication = "340" => "訂正別途買付け禁止の特例を受けるための申出書",
        LargeShareholdingReport = "350" => "大量保有報告書",
        AmendedLargeShareholdingReport = "360" => "訂正大量保有報告書",
        RecordDateNotice = "370" => "基準日の届出書",
        ChangeNotice = "380" => "変更の届出書",
    }
}

named_code_enum! {
    /// Cabinet Office Ordinance (`ordinanceCode`, 府令コード).
    OrdinanceCode {
        CorporateDisclosure = "010" => "企業内容等の開示に関する内閣府令",
        InternalControl = "015" => "財務計算に関する書類その他の情報の適正性を確保するための体制に関する内閣府令",
        ForeignBonds = "020" => "外国債等の発行者の開示に関する内閣府令",
        SpecifiedSecurities = "030" => "特定有価証券の内容等の開示に関する内閣府令",
        TenderOfferByNonIssuer = "040" => "発行者以外の者による株券等の公開買付けの開示に関する内閣府令",
        TenderOfferByIssuer = "050" => "発行者による上場株券等の公開買付けの開示に関する内閣府令",
        LargeShareholding = "060" => "株券等の大量保有の状況の開示に関する内閣府令",
    }
}

code_enum! {
    /// Withdrawal status (`withdrawalStatus`, 取下区分).
    WithdrawalStatus {
        /// `0`: not withdrawn.
        NotWithdrawn = "0",
        /// `1`: this entry is a withdrawal notice (取下書).
        WithdrawalNotice = "1",
        /// `2`: this document was withdrawn and can no longer be downloaded.
        Withdrawn = "2",
    }
}

code_enum! {
    /// Correction by a Local Finance Bureau officer (`docInfoEditStatus`, 書類情報修正区分).
    DocInfoEditStatus {
        /// `0`: not corrected.
        NotEdited = "0",
        /// `1`: this entry records a correction and holds the corrected values.
        EditNotice = "1",
        /// `2`: this document's information was corrected later.
        Edited = "2",
    }
}

code_enum! {
    /// Non-disclosure by a Local Finance Bureau officer (`disclosureStatus`, 開示不開示区分).
    DisclosureStatus {
        /// `0`: disclosed normally.
        Disclosed = "0",
        /// `1`: this entry records the start of non-disclosure.
        NonDisclosureStarted = "1",
        /// `2`: this document is not disclosed. Downloads return a notice PDF.
        NonDisclosed = "2",
        /// `3`: this entry records the end of non-disclosure.
        NonDisclosureLifted = "3",
    }
}

code_enum! {
    /// Public inspection status (`legalStatus`, 縦覧区分).
    LegalStatus {
        /// `0`: inspection period over, withdrawn, or otherwise not viewable.
        Expired = "0",
        /// `1`: under statutory public inspection.
        UnderInspection = "1",
        /// `2`: statutory period over, still viewable during the extension period.
        Extended = "2",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_codes_round_trip() {
        for code in DocTypeCode::ALL {
            assert_eq!(&DocTypeCode::from(code.code()), code);
            assert!(code.name().is_some());
        }
        for code in OrdinanceCode::ALL {
            assert_eq!(&OrdinanceCode::from(code.code()), code);
            assert!(code.name().is_some());
        }
        for code in WithdrawalStatus::ALL {
            assert_eq!(&WithdrawalStatus::from(code.code()), code);
        }
        for code in DocInfoEditStatus::ALL {
            assert_eq!(&DocInfoEditStatus::from(code.code()), code);
        }
        for code in DisclosureStatus::ALL {
            assert_eq!(&DisclosureStatus::from(code.code()), code);
        }
        for code in LegalStatus::ALL {
            assert_eq!(&LegalStatus::from(code.code()), code);
        }
        assert_eq!(DocTypeCode::ALL.len(), 42);
        assert_eq!(OrdinanceCode::ALL.len(), 7);
    }

    #[test]
    fn unknown_codes_are_kept() {
        let code = DocTypeCode::from("999");
        assert_eq!(code, DocTypeCode::Other("999".into()));
        assert_eq!(code.code(), "999");
        assert_eq!(code.name(), None);
        assert_eq!(OrdinanceCode::from("070").name(), None);
        assert_eq!(LegalStatus::from("9").to_string(), "9");
    }

    #[test]
    fn serde_uses_the_code() {
        let code: DocTypeCode = serde_json::from_str(r#""180""#).unwrap();
        assert_eq!(code, DocTypeCode::ExtraordinaryReport);
        assert_eq!(serde_json::to_string(&code).unwrap(), r#""180""#);
        assert!(serde_json::from_str::<DocTypeCode>("180").is_err());
    }
}
