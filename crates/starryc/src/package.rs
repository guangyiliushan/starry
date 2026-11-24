//! Path-based package graph v1.
//!
//! The graph owns three trust boundaries:
//! - the strict `starry.toml` subset,
//! - canonical local path resolution, and
//! - deterministic cycle/order decisions.
//!
//! Source lowering still merges packages into one text buffer, but every merged
//! file carries the `PackageId` that owns it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use ast::syntax::decl::{DeclKind, ImportKind};

const MANIFEST_NAME: &str = "starry.toml";

/// Stable, graph-local package identity. Ids are assigned only after a
/// manifest has passed dependency-key and exact-version validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageManifest {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencySpec {
    pub key: String,
    pub path: String,
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct Package {
    pub root: PathBuf,
    pub manifest: PackageManifest,
    pub dependencies: Vec<DependencySpec>,
    /// False for the synthetic implicit root package (no starry.toml).
    pub has_manifest: bool,
}

#[derive(Debug)]
pub struct PackageGraph {
    packages: Vec<Package>,
    by_name: BTreeMap<String, PackageId>,
    root: PackageId,
}

impl PackageGraph {
    pub fn root(&self) -> PackageId {
        self.root
    }

    pub fn package(&self, id: PackageId) -> &Package {
        &self.packages[id.0 as usize]
    }
    pub fn package_id(&self, name: &str) -> Option<PackageId> {
        self.by_name.get(name).copied()
    }

    /// Deterministic dependency-first postorder from the entry package.
    /// Edges and package names use ordered containers, so filesystem read
    /// order cannot influence the result.
    pub fn order(&self, root: PackageId) -> Vec<PackageId> {
        enum Visit {
            Enter(PackageId),
            Emit(PackageId),
        }
        let mut done = BTreeSet::new();
        let mut order = Vec::new();
        let mut stack = vec![Visit::Enter(root)];
        while let Some(state) = stack.pop() {
            match state {
                Visit::Emit(id) => {
                    if done.insert(id) {
                        order.push(id);
                    }
                }
                Visit::Enter(id) => {
                    if done.contains(&id) {
                        continue;
                    }
                    stack.push(Visit::Emit(id));
                    let package = self.package(id);
                    for dep in package.dependencies.iter().rev() {
                        if let Some(&target) = self.by_name.get(&dep.key) {
                            stack.push(Visit::Enter(target));
                        }
                    }
                }
            }
        }
        order
    }
}

/// A manifest parser diagnostic with a stable source position.
#[derive(Debug)]
pub struct ManifestDiagnostic {
    path: PathBuf,
    line: usize,
    col: usize,
    message: String,
}

impl fmt::Display for ManifestDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: {}",
            self.path.display(),
            self.line,
            self.col,
            self.message
        )
    }
}

#[derive(Debug)]
pub enum ManifestError {
    Read {
        path: PathBuf,
        message: String,
    },
    Invalid {
        diagnostics: Vec<ManifestDiagnostic>,
    },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, message } => write!(f, "{}: {message}", path.display()),
            Self::Invalid { diagnostics } => {
                for (i, diagnostic) in diagnostics.iter().enumerate() {
                    if i != 0 {
                        writeln!(f)?;
                    }
                    write!(f, "{diagnostic}")?;
                }
                Ok(())
            }
        }
    }
}

fn line_col(raw: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut col = 1usize;
    for (i, ch) in raw.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

struct ManifestParser<'a> {
    path: &'a std::path::Path,
    text: &'a str,
    diagnostics: Vec<ManifestDiagnostic>,
    section: Option<&'static str>,
    package: Option<PackageManifest>,
    dependencies: BTreeMap<String, DependencySpec>,
    package_seen: bool,
    line_no: usize,
}

impl<'a> ManifestParser<'a> {
    fn error(&mut self, raw: &str, offset: usize, message: impl Into<String>) {
        let (_, col) = line_col(raw, offset);
        self.diagnostics.push(ManifestDiagnostic {
            path: self.path.to_path_buf(),
            line: self.line_no,
            col,
            message: message.into(),
        });
    }

    fn parse(mut self) -> Result<Manifest, ManifestError> {
        for raw in self.text.split_inclusive('\n') {
            let content = raw.trim_end_matches(['\r', '\n']);
            let indent = content.len() - content.trim_start().len();
            let line = content.trim();
            if !line.is_empty() && !line.starts_with('#') {
                self.parse_line(line, indent);
            }
            self.line_no += 1;
        }
        if !self.package_seen {
            self.error(self.text, self.text.len(), "missing [package] table");
        }
        let empty_name = self.package.as_ref().is_some_and(|p| p.name.is_empty());
        let empty_version = self.package.as_ref().is_some_and(|p| p.version.is_empty());
        if empty_name {
            self.error(self.text, 0, "[package] name must not be empty");
        }
        if empty_version {
            self.error(self.text, 0, "[package] version must not be empty");
        }
        if self.diagnostics.is_empty() {
            let package = self.package.expect("package validated above");
            let dependencies = self.dependencies.into_values().collect();
            Ok(Manifest {
                package,
                dependencies,
            })
        } else {
            Err(ManifestError::Invalid {
                diagnostics: self.diagnostics,
            })
        }
    }

    fn parse_line(&mut self, line: &str, indent: usize) {
        if line.starts_with('[') {
            if !line.ends_with(']') || line[1..line.len() - 1].trim().is_empty() {
                self.error(line, indent, "expected [package] or [dependencies]");
                return;
            }
            let table = &line[1..line.len() - 1];
            match table {
                "package" | "dependencies" => {
                    let duplicate = self.section == Some(table)
                        || (table == "package" && self.package_seen)
                        || (table == "dependencies" && !self.dependencies.is_empty());
                    if duplicate {
                        self.error(line, indent, format!("duplicate [{table}] table"));
                    }
                    if table == "package" {
                        self.package_seen = true;
                    }
                    self.section = Some(if table == "package" {
                        "package"
                    } else {
                        "dependencies"
                    });
                }
                _ => self.error(line, indent, format!("unknown table [{table}]")),
            }
            return;
        }

        let Some(section) = self.section else {
            self.error(line, indent, "key is outside [package] or [dependencies]");
            return;
        };
        let Some(eq) = line.find('=') else {
            self.error(line, indent, "expected key = value");
            return;
        };
        let key = line[..eq].trim();
        let value = line[eq + 1..].trim();
        if key.is_empty() {
            self.error(line, indent, "empty key");
            return;
        }
        let value_offset = indent + eq + 1 + (line[eq + 1..].len() - value.len());
        match section {
            "package" => self.parse_package_key(key, value, value_offset),
            "dependencies" => self.parse_dependency(key, value, value_offset),
            _ => unreachable!("section is constrained to package/dependencies"),
        }
    }

    fn parse_package_key(&mut self, key: &str, value: &str, value_offset: usize) {
        let Some((string, _used)) = self.parse_string(value, value_offset) else {
            return;
        };
        let duplicate = match key {
            "name" => self.package.as_ref().is_some_and(|p| !p.name.is_empty()),
            "version" => self.package.as_ref().is_some_and(|p| !p.version.is_empty()),
            _ => false,
        };
        if !matches!(key, "name" | "version") {
            self.error(value, 0, format!("unknown [package] key `{key}`"));
            return;
        }
        if duplicate {
            self.error(value, 0, format!("duplicate [package] key `{key}`"));
            return;
        }
        let package = self.package.get_or_insert(PackageManifest {
            name: String::new(),
            version: String::new(),
        });
        match key {
            "name" => package.name = string,
            _ => package.version = string,
        }
    }

    fn parse_dependency(&mut self, key: &str, value: &str, value_offset: usize) {
        let Some((path, version)) = self.parse_inline_dependency(value, value_offset) else {
            return;
        };
        if self.dependencies.contains_key(key) {
            self.error(value, value_offset, format!("duplicate dependency `{key}`"));
            return;
        }
        self.dependencies.insert(
            key.to_string(),
            DependencySpec {
                key: key.to_string(),
                path,
                version,
            },
        );
    }

    fn parse_inline_dependency(
        &mut self,
        value: &str,
        value_offset: usize,
    ) -> Option<(String, String)> {
        let mut rest = value.trim_start();
        let mut offset = value_offset;
        let mut path: Option<String> = None;
        let mut version: Option<String> = None;
        if !rest.starts_with('{') {
            self.error(
                value,
                offset,
                "dependency value must be { path = \"...\", version = \"...\" }",
            );
            return None;
        }
        rest = rest[1..].trim_start();
        offset += 1;
        loop {
            if rest.starts_with('}') {
                rest = rest[1..].trim_start();
                offset += 1;
                if !rest.is_empty() {
                    self.error(value, offset, "unexpected text after inline table");
                    return None;
                }
                if path.is_none() || version.is_none() {
                    self.error(value, offset, "dependency needs path and version");
                    return None;
                }
                return Some((
                    path.expect("checked above"),
                    version.expect("checked above"),
                ));
            }
            let Some(eq) = rest.find('=') else {
                self.error(value, offset, "expected inline key = value");
                return None;
            };
            let key = rest[..eq].trim();
            if key.is_empty() {
                self.error(value, offset, "empty inline key");
                return None;
            }
            offset += eq + 1;
            rest = rest[eq + 1..].trim_start();
            let (parsed, used) = self.parse_string(rest, offset)?;
            match (key, &path, &version) {
                ("path", Some(_), _) | ("version", _, Some(_)) => {
                    self.error(value, offset, format!("duplicate dependency key `{key}`"));
                    return None;
                }
                ("path", None, _) => path = Some(parsed),
                ("version", _, None) => version = Some(parsed),
                _ => {
                    self.error(value, offset, format!("unknown dependency key `{key}`"));
                    return None;
                }
            }
            offset += used;
            rest = rest[used..].trim_start();
            if rest.starts_with(',') {
                rest = rest[1..].trim_start();
                offset += 1;
                if rest.starts_with('}') {
                    self.error(value, offset, "trailing comma in inline table");
                    return None;
                }
                continue;
            }
            if !rest.starts_with('}') {
                self.error(value, offset, "expected , or } in inline table");
                return None;
            }
        }
    }

    fn parse_string(&mut self, value: &str, offset: usize) -> Option<(String, usize)> {
        if !value.starts_with('"') {
            self.error(value, offset, "expected a double-quoted string");
            return None;
        }
        let Some(len) = value[1..].find('"').map(|i| i + 2) else {
            self.error(value, offset, "unterminated string");
            return None;
        };
        let inner = &value[1..len - 1];
        if inner.contains('\\') || inner.contains('\n') || inner.contains('\r') {
            self.error(
                value,
                offset,
                "string escapes are not in the manifest subset",
            );
            return None;
        }
        Some((inner.to_string(), len))
    }
}

#[derive(Debug)]
pub struct Manifest {
    pub package: PackageManifest,
    pub dependencies: Vec<DependencySpec>,
}

pub fn parse_manifest(path: &Path) -> Result<Manifest, ManifestError> {
    let text = std::fs::read_to_string(path).map_err(|e| ManifestError::Read {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    ManifestParser {
        path,
        text: &text,
        diagnostics: Vec::new(),
        section: None,
        package: None,
        dependencies: BTreeMap::new(),
        package_seen: false,
        line_no: 1,
    }
    .parse()
}

#[derive(Debug)]
pub enum PackageGraphError {
    Manifest(ManifestError),
    Io {
        path: PathBuf,
        message: String,
    },
    ManifestRequired {
        path: PathBuf,
    },
    DuplicateName {
        name: String,
        first: PathBuf,
        second: PathBuf,
    },
    DependencyNameMismatch {
        dependency: String,
        package_name: String,
    },
    ExactVersionMismatch {
        dependency: String,
        requested: String,
        actual: String,
    },
    DependencyCycle {
        path: Vec<String>,
    },
    ImportCycle {
        path: Vec<String>,
    },
    NoSources {
        package: String,
        root: PathBuf,
    },
}

impl fmt::Display for PackageGraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manifest(e) => write!(f, "{e}"),
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
            Self::ManifestRequired { path } => write!(
                f,
                "{}: dependency requires {}",
                path.display(),
                MANIFEST_NAME
            ),
            Self::DuplicateName {
                name,
                first,
                second,
            } => write!(
                f,
                "duplicate package identity `{name}` at {} and {}",
                first.display(),
                second.display()
            ),
            Self::DependencyNameMismatch {
                dependency,
                package_name,
            } => write!(
                f,
                "dependency key `{dependency}` does not match package name `{package_name}`"
            ),
            Self::ExactVersionMismatch {
                dependency,
                requested,
                actual,
            } => write!(
                f,
                "dependency `{dependency}` requests version {requested}, package has {actual}"
            ),
            Self::DependencyCycle { path } => write!(f, "dependency cycle: {}", path.join(" -> ")),
            Self::ImportCycle { path } => write!(f, "import cycle: {}", path.join(" -> ")),
            Self::NoSources { package, root } => {
                write!(
                    f,
                    "package `{package}` has no .starry sources at {}",
                    root.display()
                )
            }
        }
    }
}

fn canonical_root(path: &Path) -> Result<PathBuf, PackageGraphError> {
    let canonical = std::fs::canonicalize(path).map_err(|e| PackageGraphError::Io {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    let raw = canonical.to_string_lossy();
    let portable = if let Some(rest) = raw.strip_prefix(r"\\?\\UNC\\") {
        format!(r"\\\\{rest}")
    } else if let Some(rest) = raw.strip_prefix(r"\\?\\") {
        rest.to_string()
    } else {
        raw.into_owned()
    };
    Ok(PathBuf::from(portable))
}

struct GraphBuilder {
    packages: Vec<Package>,
    by_name: BTreeMap<String, PackageId>,
    loading: BTreeSet<String>,
    path: Vec<String>,
    depth: usize,
}

impl GraphBuilder {
    fn load(
        &mut self,
        root: &Path,
        expected_name: &str,
        requested_version: Option<&str>,
        manifest: Option<Manifest>,
    ) -> Result<PackageId, PackageGraphError> {
        if self.depth >= 128 {
            return Err(PackageGraphError::Io {
                path: root.to_path_buf(),
                message: format!("dependency graph exceeds maximum depth 128 at `{expected_name}`",),
            });
        }
        let canonical = canonical_root(root)?;
        if self.loading.contains(expected_name) {
            let mut cycle = self.path.clone();
            cycle.push(expected_name.to_string());
            return Err(PackageGraphError::DependencyCycle { path: cycle });
        }
        let (manifest, has_manifest) = match manifest {
            Some(manifest) => (manifest, true),
            None => {
                let manifest_path = canonical.join(MANIFEST_NAME);
                if !manifest_path.is_file() {
                    if self.packages.is_empty() {
                        (
                            Manifest {
                                package: PackageManifest {
                                    name: expected_name.to_string(),
                                    version: "0".to_string(),
                                },
                                dependencies: Vec::new(),
                            },
                            false,
                        )
                    } else {
                        return Err(PackageGraphError::ManifestRequired { path: canonical });
                    }
                } else {
                    (
                        parse_manifest(&manifest_path).map_err(PackageGraphError::Manifest)?,
                        true,
                    )
                }
            }
        };
        let manifest_deps = manifest.dependencies;
        let manifest = manifest.package;
        if manifest.name != expected_name {
            return Err(PackageGraphError::DependencyNameMismatch {
                dependency: expected_name.to_string(),
                package_name: manifest.name.clone(),
            });
        }
        if let Some(requested) = requested_version
            && requested != manifest.version
        {
            return Err(PackageGraphError::ExactVersionMismatch {
                dependency: expected_name.to_string(),
                requested: requested.to_string(),
                actual: manifest.version.clone(),
            });
        }
        if let Some(&existing) = self.by_name.get(&manifest.name) {
            let first = self.packages[existing.0 as usize].root.clone();
            if first != canonical {
                return Err(PackageGraphError::DuplicateName {
                    name: manifest.name.clone(),
                    first,
                    second: canonical,
                });
            }
            return Ok(existing);
        }
        self.loading.insert(manifest.name.clone());
        self.path.push(manifest.name.clone());
        self.depth += 1;
        let mut dependencies = Vec::new();
        for spec in &manifest_deps {
            if Path::new(&spec.path).is_absolute() {
                return Err(PackageGraphError::Io {
                    path: canonical,
                    message: format!("dependency path must be relative: {}", spec.path),
                });
            }
            let target = canonical.join(&spec.path);
            self.load(&target, &spec.key, Some(&spec.version), None)?;
            dependencies.push(spec.clone());
        }
        self.depth -= 1;
        self.path.pop();
        self.loading.remove(&manifest.name);
        let id = PackageId(u32::try_from(self.packages.len()).map_err(|_| {
            PackageGraphError::DependencyCycle {
                path: self.path.clone(),
            }
        })?);
        self.by_name.insert(manifest.name.clone(), id);
        self.packages.push(Package {
            root: canonical,
            manifest,
            dependencies,
            has_manifest,
        });
        Ok(id)
    }
}

pub fn load_graph(
    entry_root: &Path,
    entry_manifest: Option<Manifest>,
) -> Result<PackageGraph, PackageGraphError> {
    let name = entry_manifest
        .as_ref()
        .map(|m| m.package.name.clone())
        .unwrap_or_else(|| {
            entry_root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "starry-app".to_string())
        });
    let version = entry_manifest.as_ref().map(|m| m.package.version.clone());
    let mut builder = GraphBuilder {
        packages: Vec::new(),
        by_name: BTreeMap::new(),
        loading: BTreeSet::new(),
        path: Vec::new(),
        depth: 0,
    };
    let root = builder.load(entry_root, &name, version.as_deref(), entry_manifest)?;
    Ok(PackageGraph {
        packages: builder.packages,
        by_name: builder.by_name,
        root,
    })
}

/// One merged source unit plus its compiler-derived module path.
pub struct ImportUnit<'a> {
    pub module_path: &'a str,
    pub source: &'a str,
}

/// Detect module-level import cycles across merged source units. Target
/// identity is the longest module-path prefix of the import, so both
/// `import util.f` and `import demo.core.f` resolve to their owner.
pub fn check_import_cycles(units: &[ImportUnit<'_>]) -> Result<(), PackageGraphError> {
    let mut packages = BTreeSet::new();
    for unit in units {
        packages.insert(unit.module_path.to_string());
    }
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for unit in units {
        let parsed = parser::parse_source(unit.source).map_err(|e| PackageGraphError::Io {
            path: PathBuf::from("<merged source>"),
            message: e.to_string(),
        })?;
        let current = unit.module_path;
        for item in &parsed.ast.items {
            let DeclKind::Import(import) = &item.kind else {
                continue;
            };
            if let ImportKind::Qualified { path, .. } = &import.kind {
                let segments: Vec<&str> = path
                    .segments
                    .iter()
                    .map(|segment| parsed.interner.resolve(segment.name))
                    .collect();
                let Some(target) = longest_package_prefix(&segments, &packages) else {
                    continue;
                };
                if target == current {
                    return Err(PackageGraphError::ImportCycle {
                        path: vec![current.to_string(), target],
                    });
                }
                edges
                    .entry(current.to_string())
                    .or_default()
                    .insert(target.to_string());
            }
        }
    }
    let mut loading = BTreeSet::new();
    let mut done = BTreeSet::new();
    let mut stack: Vec<String> = packages.iter().cloned().collect();
    stack.reverse();
    while let Some(package) = stack.pop() {
        visit_import(&package, &edges, &mut Vec::new(), &mut loading, &mut done)?;
    }
    Ok(())
}
fn visit_import(
    package: &str,
    edges: &BTreeMap<String, BTreeSet<String>>,
    path: &mut Vec<String>,
    loading: &mut BTreeSet<String>,
    done: &mut BTreeSet<String>,
) -> Result<(), PackageGraphError> {
    if loading.contains(package) {
        let start = path.iter().position(|p| p == package).unwrap_or(0);
        let mut cycle: Vec<String> = path[start..].to_vec();
        cycle.push(package.to_string());
        return Err(PackageGraphError::ImportCycle { path: cycle });
    }
    if !done.insert(package.to_string()) {
        return Ok(());
    }
    loading.insert(package.to_string());
    path.push(package.to_string());
    if let Some(targets) = edges.get(package) {
        for target in targets {
            visit_import(target, edges, path, loading, done)?;
        }
    }
    path.pop();
    loading.remove(package);
    Ok(())
}

fn longest_package_prefix(segments: &[&str], packages: &BTreeSet<String>) -> Option<String> {
    (1..=segments.len())
        .rev()
        .map(|end| segments[..end].join("."))
        .find(|candidate| packages.contains(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().expect("relative path has parent")).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn manifest(name: &str, version: &str, deps: &str) -> String {
        format!("[package]\nname = \"{name}\"\nversion = \"{version}\"\n\n[dependencies]\n{deps}")
    }

    fn package(root: &Path, name: &str, version: &str, deps: &str, body: &str) {
        write(root, "starry.toml", &manifest(name, version, deps));
        write(root, "lib.starry", body);
    }

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("starry_pkg_graph_{}_{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn manifest_unknown_keys_are_rejected_with_positions() {
        let dir = temp("unknown");
        let path = dir.join(MANIFEST_NAME);
        std::fs::write(
            &path,
            "[package]\nname = \"a\"\nversion = \"1\"\nextra = \"x\"\n",
        )
        .unwrap();
        let err = parse_manifest(&path).unwrap_err();
        let rendered = err.to_string();
        assert!(rendered.contains("4:1"), "{rendered}");
        assert!(rendered.contains("unknown [package] key"), "{rendered}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nested_graph_orders_dependencies_first() {
        let dir = temp("nested");
        let core = dir.join("core");
        let math = dir.join("math");
        let app = dir.join("app");
        package(&core, "core", "1.0.0", "", "fun one(): int = 1\n");
        package(
            &math,
            "math",
            "1.2.0",
            "core = { path = \"../core\", version = \"1.0.0\" }",
            "fun two(): int = 2\n",
        );
        package(
            &app,
            "app",
            "2.0.0",
            "math = { path = \"../math\", version = \"1.2.0\" }",
            "fun main(): int = 0\n",
        );
        let graph = load_graph(&app, None).unwrap();
        eprintln!(
            "core={:?} math={:?} app={:?}",
            graph.package_id("core"),
            graph.package_id("math"),
            graph.package_id("app")
        );
        let order = graph.order(graph.root());
        let names: Vec<_> = order
            .iter()
            .map(|id| graph.package(*id).manifest.name.as_str())
            .collect();
        assert_eq!(names, ["core", "math", "app"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dependency_cycles_report_the_complete_path() {
        let dir = temp("cycle");
        let a = dir.join("a");
        let b = dir.join("b");
        package(
            &a,
            "a",
            "1.0.0",
            "b = { path = \"../b\", version = \"1.0.0\" }",
            "",
        );
        package(
            &b,
            "b",
            "1.0.0",
            "a = { path = \"../a\", version = \"1.0.0\" }",
            "",
        );
        let err = load_graph(&a, None).unwrap_err();
        assert!(err.to_string().contains("a -> b -> a"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_cycles_include_both_edges() {
        let err = check_import_cycles(&[
            ImportUnit {
                module_path: "a",
                source: "import b.f",
            },
            ImportUnit {
                module_path: "b",
                source: "import a.g",
            },
        ])
        .unwrap_err();
        assert!(
            err.to_string().contains("import cycle: a -> b -> a"),
            "{err}"
        );
    }
}
