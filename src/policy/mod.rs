//! Policy subsystem for air-gapped code auditing, clause parsing, and obligation structuring.

pub mod clause;
pub mod linker;
pub mod parser;
pub mod structurer;

pub use clause::{
    ObligationType, PolicyClause, PolicyDocument, PolicyFormat, PolicyObligation, Severity,
};
pub use linker::{link_document, ComplianceLink, ComplianceReport, ComplianceStatus, EvidenceNode};
pub use parser::{parse_policy_file, PolicyParser};
pub use structurer::ObligationStructurer;
