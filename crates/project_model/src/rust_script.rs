//! FIXME: write short doc here

use lsp_types::Range;
use paths::AbsPathBuf;

/// Roots and crates that compose this Rust project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustScriptMeta {
    pub script_file: AbsPathBuf,
    pub manifest_span: Option<Range>,
}
