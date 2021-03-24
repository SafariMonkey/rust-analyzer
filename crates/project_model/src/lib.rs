//! FIXME: write short doc here

mod cargo_workspace;
mod cfg_flag;
mod project_json;
mod rust_script;
mod sysroot;
mod workspace;
mod rustc_cfg;
mod build_data;

use std::{
    fs::{read_dir, File, ReadDir},
    io::{self, BufRead, Read},
    process::Command,
};

use anyhow::{bail, Context, Result};
use paths::{AbsPath, AbsPathBuf};
use rustc_hash::FxHashSet;

pub use crate::{
    build_data::{BuildDataCollector, BuildDataResult},
    cargo_workspace::{
        CargoConfig, CargoWorkspace, Package, PackageData, PackageDependency, RustcSource, Target,
        TargetData, TargetKind,
    },
    project_json::{ProjectJson, ProjectJsonData},
    rust_script::RustScriptMeta,
    sysroot::Sysroot,
    workspace::{PackageRoot, ProjectWorkspace},
};

pub use proc_macro_api::ProcMacroClient;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum ProjectManifest {
    ProjectJson(AbsPathBuf),
    CargoToml(AbsPathBuf),
    RustScript(AbsPathBuf),
}

impl ProjectManifest {
    pub fn from_manifest_file(path: AbsPathBuf) -> Result<ProjectManifest> {
        if path.ends_with("rust-project.json") {
            return Ok(ProjectManifest::ProjectJson(path));
        }
        if path.ends_with("Cargo.toml") {
            return Ok(ProjectManifest::CargoToml(path));
        }
        bail!("project root must point to Cargo.toml or rust-project.json: {}", path.display())
    }

    pub fn discover_single(path: &AbsPath) -> Result<ProjectManifest> {
        let mut candidates = ProjectManifest::discover(path)?;
        let res = match candidates.pop() {
            None => bail!("no projects"),
            Some(it) => it,
        };

        if !candidates.is_empty() {
            bail!("more than one project")
        }
        Ok(res)
    }

    pub fn discover(path: &AbsPath) -> io::Result<Vec<ProjectManifest>> {
        if let Some(project_json) = find_in_parent_dirs(path, "rust-project.json") {
            return Ok(vec![ProjectManifest::ProjectJson(project_json)]);
        }
        let cargo_tomls =
            find_cargo_toml(path).map(|path| path.into_iter().map(ProjectManifest::CargoToml));
        let rust_scripts = read_dir(path)
            .map(find_rust_scripts)
            .map(|path| path.into_iter().map(ProjectManifest::RustScript));

        return match (cargo_tomls, rust_scripts) {
            (Ok(cargo_tomls), Ok(rust_scripts)) => Ok(cargo_tomls.chain(rust_scripts).collect()),
            (Ok(cargo_tomls), Err(_)) => Ok((cargo_tomls).collect()),
            (Err(_), Ok(rust_scripts)) => Ok((rust_scripts).collect()),
            (Err(cargo_toml_err), Err(_)) => Err(cargo_toml_err),
        };

        fn find_cargo_toml(path: &AbsPath) -> io::Result<Vec<AbsPathBuf>> {
            match find_in_parent_dirs(path, "Cargo.toml") {
                Some(it) => Ok(vec![it]),
                None => Ok(find_cargo_toml_in_child_dir(read_dir(path)?)),
            }
        }

        fn find_in_parent_dirs(path: &AbsPath, target_file_name: &str) -> Option<AbsPathBuf> {
            if path.ends_with(target_file_name) {
                return Some(path.to_path_buf());
            }

            let mut curr = Some(path);

            while let Some(path) = curr {
                let candidate = path.join(target_file_name);
                if candidate.exists() {
                    return Some(candidate);
                }
                curr = path.parent();
            }

            None
        }

        fn find_cargo_toml_in_child_dir(entities: ReadDir) -> Vec<AbsPathBuf> {
            // Only one level down to avoid cycles the easy way and stop a runaway scan with large projects
            entities
                .filter_map(Result::ok)
                .map(|it| it.path().join("Cargo.toml"))
                .filter(|it| it.exists())
                .map(AbsPathBuf::assert)
                .collect()
        }

        fn find_rust_scripts(entities: ReadDir) -> Vec<AbsPathBuf> {
            // Only one level down to avoid cycles the easy way and stop a runaway scan with large projects
            entities
                .filter_map(Result::ok)
                .map(|it| it.path().join("Cargo.toml"))
                .filter(|it| it.exists())
                .map(AbsPathBuf::assert)
                .filter_map(is_rust_script)
                .collect()
        }

        fn is_rust_script(file_path: AbsPathBuf) -> Option<AbsPathBuf> {
            if file_path.is_dir() {
                return None;
            }
            let ext = file_path.extension().map(|ext| ext.to_str()).flatten();
            if !matches!(ext, Some("ers") | Some("rs")) {
                return None;
            }
            // read the shebang first, then the rest of the line
            // to avoid reading #![...] as a shebang, check next character too
            let mut file = File::open(&file_path).ok()?;
            let mut shebang = [0u8; 3];
            file.read_exact(&mut shebang).ok()?;
            match shebang {
                [b'#', b'!', next] if next == b'/' || char::from(next).is_ascii_whitespace() => {
                    let first_line_minus_shebang = io::BufReader::new(file).lines().next()?.ok()?;
                    first_line_minus_shebang.contains("rust-script").then(|| file_path)
                }
                _ => None,
            }
        }
    }

    pub fn discover_all(paths: &[impl AsRef<AbsPath>]) -> Vec<ProjectManifest> {
        let mut res = paths
            .iter()
            .filter_map(|it| ProjectManifest::discover(it.as_ref()).ok())
            .flatten()
            .collect::<FxHashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        res.sort();
        res
    }
}

fn utf8_stdout(mut cmd: Command) -> Result<String> {
    let output = cmd.output().with_context(|| format!("{:?} failed", cmd))?;
    if !output.status.success() {
        match String::from_utf8(output.stderr) {
            Ok(stderr) if !stderr.is_empty() => {
                bail!("{:?} failed, {}\nstderr:\n{}", cmd, output.status, stderr)
            }
            _ => bail!("{:?} failed, {}", cmd, output.status),
        }
    }
    let stdout = String::from_utf8(output.stdout)?;
    Ok(stdout.trim().to_string())
}
