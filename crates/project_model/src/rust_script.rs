//! FIXME: write short doc here

use std::path::PathBuf;

use base_db::{CrateDisplayName, CrateId, CrateName, Dependency, Edition};
use paths::{AbsPath, AbsPathBuf};
use rustc_hash::FxHashMap;
use serde::{de, Deserialize};

use crate::cfg_flag::CfgFlag;

/// Roots and crates that compose this Rust project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustScriptMeta {
    pub script_file: AbsPathBuf,
}
