#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use keith_agent_types::{ProfileId, UtcTimestamp};
use keith_retrieval::{
    RetrievalError, RetrievalService, SearchResult, SearchSourceKind, SourceInput,
};
use keith_workspace::{
    EditOutcome, FileToken, PersonalWorkspace, PersonalWorkspaceError, WorkspaceActor,
    WorkspaceEvent,
};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgePage {
    pub path: String,
    pub title: String,
    pub content: String,
    pub token: FileToken,
    pub links: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrokenLink {
    pub source: String,
    pub target: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeGraph {
    pub pages: BTreeMap<String, KnowledgePage>,
    pub edges: BTreeMap<String, BTreeSet<String>>,
    pub broken: Vec<BrokenLink>,
}

#[derive(Debug, Error)]
pub enum KnowledgeError {
    #[error("knowledge I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("knowledge workspace failed: {0}")]
    Workspace(#[from] PersonalWorkspaceError),
    #[error("knowledge retrieval failed: {0}")]
    Retrieval(#[from] RetrievalError),
    #[error("knowledge path or Markdown link is unsafe")]
    UnsafePath,
    #[error("knowledge page was not found")]
    NotFound,
    #[error("knowledge page already exists")]
    AlreadyExists,
    #[error("knowledge edit conflicted with a newer human edit at {0}")]
    Conflict(String),
    #[error("knowledge content is not valid UTF-8 Markdown")]
    InvalidMarkdown,
    #[error("knowledge state lock was poisoned")]
    LockPoisoned,
    #[error("knowledge rollback failed after: {cause}; rollback: {rollback}")]
    Rollback { cause: String, rollback: String },
}

pub struct KnowledgeService {
    workspace: PersonalWorkspace,
    retrieval: Arc<RetrievalService>,
    profile_id: ProfileId,
    serial: Mutex<()>,
}

impl KnowledgeService {
    pub fn new(
        workspace: PersonalWorkspace,
        retrieval: Arc<RetrievalService>,
        profile_id: ProfileId,
    ) -> Self {
        Self {
            workspace,
            retrieval,
            profile_id,
            serial: Mutex::new(()),
        }
    }

    /// Creates a Markdown page with an optimistic missing-file precondition.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe/existing paths, workspace conflicts, or retrieval failure.
    pub fn create(
        &self,
        path: &str,
        content: impl Into<String>,
        now: UtcTimestamp,
    ) -> Result<KnowledgePage, KnowledgeError> {
        let _guard = self.lock()?;
        let path = knowledge_path(path)?;
        let content = content.into();
        validate_markdown(&content)?;
        self.workspace.scan_external_changes(now)?;
        let expected = self.workspace.token(&path)?;
        if expected.digest.is_some() {
            return Err(KnowledgeError::AlreadyExists);
        }
        let snapshot = self
            .workspace
            .create_snapshot("before knowledge create", now)?;
        ensure_parent(&self.workspace.layout().root, &path)?;
        let result = self
            .workspace
            .edit(
                WorkspaceActor::Agent,
                &path,
                &expected,
                content.as_bytes(),
                now,
            )
            .map_err(KnowledgeError::from)
            .and_then(|outcome| written_token(outcome, &path))
            .and_then(|token| {
                self.index_page(&path, &content, &token, now)?;
                Self::page_from_parts(&path, content.clone(), token)
            });
        self.finish_transaction(result, &snapshot.id, now)
    }

    /// Reads a current page after validating external changes and profile-root isolation.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe/missing/corrupt pages or workspace scan failure.
    pub fn inspect(&self, path: &str, now: UtcTimestamp) -> Result<KnowledgePage, KnowledgeError> {
        let _guard = self.lock()?;
        let path = knowledge_path(path)?;
        self.scan(now)?;
        self.read_page(&path)
    }

    /// Replaces a page only when the supplied workspace version token is current.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe paths, conflicts, invalid Markdown, or retrieval failure.
    pub fn update(
        &self,
        path: &str,
        expected: &FileToken,
        content: impl Into<String>,
        now: UtcTimestamp,
    ) -> Result<KnowledgePage, KnowledgeError> {
        let _guard = self.lock()?;
        let path = knowledge_path(path)?;
        let content = content.into();
        validate_markdown(&content)?;
        self.scan(now)?;
        let snapshot = self
            .workspace
            .create_snapshot("before knowledge update", now)?;
        let result = self
            .workspace
            .edit(
                WorkspaceActor::Agent,
                &path,
                expected,
                content.as_bytes(),
                now,
            )
            .map_err(KnowledgeError::from)
            .and_then(|outcome| written_token(outcome, &path))
            .and_then(|token| {
                self.index_page(&path, &content, &token, now)?;
                Self::page_from_parts(&path, content.clone(), token)
            });
        self.finish_transaction(result, &snapshot.id, now)
    }

    /// Deletes a page with an exact preimage and removes its derived retrieval projections.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe/missing paths, conflicts, or persistence failure.
    pub fn delete(
        &self,
        path: &str,
        expected: &FileToken,
        now: UtcTimestamp,
    ) -> Result<(), KnowledgeError> {
        let _guard = self.lock()?;
        let path = knowledge_path(path)?;
        self.scan(now)?;
        let snapshot = self
            .workspace
            .create_snapshot("before knowledge delete", now)?;
        let result = self
            .workspace
            .delete(WorkspaceActor::Agent, &path, expected, now)
            .map_err(KnowledgeError::from)
            .and_then(|outcome| written_token(outcome, &path).map(|_| ()))
            .and_then(|()| {
                self.retrieval
                    .remove_source(&self.profile_id, &path_string(&path))?;
                Ok(())
            });
        self.finish_transaction(result, &snapshot.id, now)
    }

    /// Atomically moves a page and repairs inbound and moved-page relative links.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe paths, target conflicts, concurrent edits, or rebuild failure.
    pub fn rename(
        &self,
        from: &str,
        to: &str,
        now: UtcTimestamp,
    ) -> Result<KnowledgePage, KnowledgeError> {
        let _guard = self.lock()?;
        let from = knowledge_path(from)?;
        let to = knowledge_path(to)?;
        if from == to {
            return self.read_page(&from);
        }
        self.scan(now)?;
        let graph = self.graph_locked()?;
        let old = graph
            .pages
            .get(&path_string(&from))
            .cloned()
            .ok_or(KnowledgeError::NotFound)?;
        if graph.pages.contains_key(&path_string(&to)) {
            return Err(KnowledgeError::AlreadyExists);
        }
        let snapshot = self
            .workspace
            .create_snapshot("before knowledge rename", now)?;
        ensure_parent(&self.workspace.layout().root, &to)?;
        let result = self
            .apply_rename(&graph, &from, &to, &old, now)
            .and_then(|()| {
                self.retrieval.rebuild_workspace(
                    &self.profile_id,
                    &self.workspace.layout().root,
                    now,
                )?;
                self.read_page(&to)
            });
        self.finish_transaction(result, &snapshot.id, now)
    }

    /// Repairs links that resolve to known old paths using one snapshot-backed transaction.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe mappings, concurrent edits, or retrieval rebuild failure.
    pub fn repair_links(
        &self,
        replacements: &BTreeMap<String, String>,
        now: UtcTimestamp,
    ) -> Result<usize, KnowledgeError> {
        let _guard = self.lock()?;
        self.scan(now)?;
        let replacements = replacements
            .iter()
            .map(|(from, to)| Ok((knowledge_path(from)?, knowledge_path(to)?)))
            .collect::<Result<BTreeMap<_, _>, KnowledgeError>>()?;
        let graph = self.graph_locked()?;
        let snapshot = self
            .workspace
            .create_snapshot("before knowledge link repair", now)?;
        let result = self
            .apply_link_repair(&graph, &replacements, now)
            .and_then(|changed| {
                self.retrieval.rebuild_workspace(
                    &self.profile_id,
                    &self.workspace.layout().root,
                    now,
                )?;
                Ok(changed)
            });
        self.finish_transaction(result, &snapshot.id, now)
    }

    /// Returns pages, resolved edges, and broken relative Markdown links.
    ///
    /// # Errors
    ///
    /// Returns an error when workspace pages cannot be scanned or decoded.
    pub fn graph(&self, now: UtcTimestamp) -> Result<KnowledgeGraph, KnowledgeError> {
        let _guard = self.lock()?;
        self.scan(now)?;
        self.graph_locked()
    }

    /// Returns pages that link to the selected page.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe paths or unreadable workspace state.
    pub fn backlinks(&self, path: &str, now: UtcTimestamp) -> Result<Vec<String>, KnowledgeError> {
        let target = path_string(&knowledge_path(path)?);
        let graph = self.graph(now)?;
        Ok(graph
            .edges
            .into_iter()
            .filter_map(|(source, targets)| targets.contains(&target).then_some(source))
            .collect())
    }

    /// Returns pages with no incoming or outgoing internal links, excluding `index.md`.
    ///
    /// # Errors
    ///
    /// Returns an error when workspace pages cannot be scanned.
    pub fn orphans(&self, now: UtcTimestamp) -> Result<Vec<String>, KnowledgeError> {
        let graph = self.graph(now)?;
        let incoming = graph
            .edges
            .values()
            .flat_map(BTreeSet::iter)
            .cloned()
            .collect::<BTreeSet<_>>();
        Ok(graph
            .pages
            .keys()
            .filter(|path| {
                path.as_str() != "knowledge/index.md"
                    && !incoming.contains(*path)
                    && graph.edges.get(*path).is_none_or(BTreeSet::is_empty)
            })
            .cloned()
            .collect())
    }

    /// Returns directly linked, backlinking, and shared-neighbor pages.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe paths or unreadable workspace state.
    pub fn related(&self, path: &str, now: UtcTimestamp) -> Result<Vec<String>, KnowledgeError> {
        let path = path_string(&knowledge_path(path)?);
        let graph = self.graph(now)?;
        let outgoing = graph.edges.get(&path).cloned().unwrap_or_default();
        let mut related = outgoing.clone();
        for (source, targets) in &graph.edges {
            if targets.contains(&path) || (!outgoing.is_empty() && !targets.is_disjoint(&outgoing))
            {
                related.insert(source.clone());
            }
        }
        related.remove(&path);
        Ok(related.into_iter().collect())
    }

    /// Searches profile-scoped knowledge through the shared hybrid retrieval service.
    ///
    /// # Errors
    ///
    /// Returns an error for empty queries or retrieval failures.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, KnowledgeError> {
        Ok(self.retrieval.search(&self.profile_id, query, limit)?)
    }

    /// Rebuilds knowledge and other supported readable sources into the derived index.
    ///
    /// # Errors
    ///
    /// Returns an error when readable source scanning or indexing fails.
    pub fn rebuild_index(&self, now: UtcTimestamp) -> Result<usize, KnowledgeError> {
        Ok(self.retrieval.rebuild_workspace(
            &self.profile_id,
            &self.workspace.layout().root,
            now,
        )?)
    }

    fn apply_rename(
        &self,
        graph: &KnowledgeGraph,
        from: &Path,
        to: &Path,
        old: &KnowledgePage,
        now: UtcTimestamp,
    ) -> Result<(), KnowledgeError> {
        let to_token = self.workspace.token(to)?;
        let moved = rewrite_links(&old.content, from, to, Some((from, to)))?;
        written_token(
            self.workspace
                .edit(WorkspaceActor::Agent, to, &to_token, moved.as_bytes(), now)?,
            to,
        )?;
        for page in graph
            .pages
            .values()
            .filter(|page| page.path != path_string(from))
        {
            let source = Path::new(&page.path);
            let replacement = rewrite_links(&page.content, source, source, Some((from, to)))?;
            if replacement != page.content {
                written_token(
                    self.workspace.edit(
                        WorkspaceActor::Agent,
                        source,
                        &page.token,
                        replacement.as_bytes(),
                        now,
                    )?,
                    source,
                )?;
            }
        }
        written_token(
            self.workspace
                .delete(WorkspaceActor::Agent, from, &old.token, now)?,
            from,
        )?;
        Ok(())
    }

    fn apply_link_repair(
        &self,
        graph: &KnowledgeGraph,
        replacements: &BTreeMap<PathBuf, PathBuf>,
        now: UtcTimestamp,
    ) -> Result<usize, KnowledgeError> {
        let mut changed = 0;
        for page in graph.pages.values() {
            let path = Path::new(&page.path);
            let replacement = rewrite_link_map(&page.content, path, replacements)?;
            if replacement != page.content {
                written_token(
                    self.workspace.edit(
                        WorkspaceActor::Agent,
                        path,
                        &page.token,
                        replacement.as_bytes(),
                        now,
                    )?,
                    path,
                )?;
                changed += 1;
            }
        }
        Ok(changed)
    }

    fn graph_locked(&self) -> Result<KnowledgeGraph, KnowledgeError> {
        let root = self.workspace.layout().root;
        let paths = collect_pages(&root, Path::new("knowledge"))?;
        let mut pages = BTreeMap::new();
        for path in paths {
            let page = self.read_page(&path)?;
            pages.insert(page.path.clone(), page);
        }
        let known = pages.keys().cloned().collect::<BTreeSet<_>>();
        let mut edges = BTreeMap::new();
        let mut broken = Vec::new();
        for page in pages.values() {
            let mut targets = BTreeSet::new();
            for link in markdown_links(&page.content) {
                if let Some(target) = resolve_link(Path::new(&page.path), &link.href)? {
                    let target = path_string(&target);
                    if known.contains(&target) {
                        targets.insert(target);
                    } else {
                        broken.push(BrokenLink {
                            source: page.path.clone(),
                            target,
                        });
                    }
                }
            }
            edges.insert(page.path.clone(), targets);
        }
        broken.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then_with(|| left.target.cmp(&right.target))
        });
        Ok(KnowledgeGraph {
            pages,
            edges,
            broken,
        })
    }

    fn read_page(&self, path: &Path) -> Result<KnowledgePage, KnowledgeError> {
        let absolute = self.workspace.layout().root.join(path);
        let content = fs::read_to_string(absolute).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                KnowledgeError::NotFound
            } else if error.kind() == std::io::ErrorKind::InvalidData {
                KnowledgeError::InvalidMarkdown
            } else {
                error.into()
            }
        })?;
        let token = self.workspace.token(path)?;
        Self::page_from_parts(path, content, token)
    }

    fn page_from_parts(
        path: &Path,
        content: String,
        token: FileToken,
    ) -> Result<KnowledgePage, KnowledgeError> {
        let links = markdown_links(&content)
            .into_iter()
            .filter_map(|link| resolve_link(path, &link.href).transpose())
            .collect::<Result<BTreeSet<_>, KnowledgeError>>()?
            .into_iter()
            .map(|path| path_string(&path))
            .collect();
        Ok(KnowledgePage {
            title: page_title(path, &content),
            path: path_string(path),
            content,
            token,
            links,
        })
    }

    fn index_page(
        &self,
        path: &Path,
        content: &str,
        token: &FileToken,
        now: UtcTimestamp,
    ) -> Result<(), KnowledgeError> {
        let version = token.digest.clone().ok_or(KnowledgeError::NotFound)?;
        self.retrieval.index_sources(
            &self.profile_id,
            &[SourceInput {
                source_path: path_string(path),
                source_version: version,
                source_kind: SearchSourceKind::Knowledge,
                modified_at: now,
                text: content.to_owned(),
            }],
        )?;
        Ok(())
    }

    fn scan(&self, now: UtcTimestamp) -> Result<(), KnowledgeError> {
        let events = self.workspace.scan_external_changes(now)?;
        if events
            .iter()
            .any(|event| matches!(event, WorkspaceEvent::Rejected { .. }))
        {
            Err(KnowledgeError::UnsafePath)
        } else {
            Ok(())
        }
    }

    fn finish_transaction<T>(
        &self,
        result: Result<T, KnowledgeError>,
        snapshot_id: &keith_agent_types::EntityId,
        now: UtcTimestamp,
    ) -> Result<T, KnowledgeError> {
        match result {
            Ok(value) => Ok(value),
            Err(cause) => {
                if let Err(rollback) =
                    self.workspace
                        .restore_snapshot(WorkspaceActor::System, snapshot_id, now)
                {
                    return Err(KnowledgeError::Rollback {
                        cause: cause.to_string(),
                        rollback: rollback.to_string(),
                    });
                }
                let _ = self.retrieval.rebuild_workspace(
                    &self.profile_id,
                    &self.workspace.layout().root,
                    now,
                );
                Err(cause)
            }
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, ()>, KnowledgeError> {
        self.serial.lock().map_err(|_| KnowledgeError::LockPoisoned)
    }
}

#[derive(Clone)]
struct MarkdownLink {
    href_range: Range<usize>,
    href: String,
}

fn knowledge_path(path: &str) -> Result<PathBuf, KnowledgeError> {
    let path = Path::new(path);
    if path.is_absolute() {
        return Err(KnowledgeError::UnsafePath);
    }
    let mut output = PathBuf::from("knowledge");
    let components = if path.starts_with("knowledge") {
        path.strip_prefix("knowledge")
            .map_err(|_| KnowledgeError::UnsafePath)?
            .components()
    } else {
        path.components()
    };
    for component in components {
        match component {
            Component::Normal(value) => output.push(value),
            _ => return Err(KnowledgeError::UnsafePath),
        }
    }
    if output == Path::new("knowledge") || output.extension().is_none_or(|value| value != "md") {
        return Err(KnowledgeError::UnsafePath);
    }
    Ok(output)
}

fn ensure_parent(root: &Path, path: &Path) -> Result<(), KnowledgeError> {
    let parent = path.parent().ok_or(KnowledgeError::UnsafePath)?;
    let mut current = root.to_path_buf();
    for component in parent.components() {
        let Component::Normal(value) = component else {
            return Err(KnowledgeError::UnsafePath);
        };
        current.push(value);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(KnowledgeError::UnsafePath);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current)?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn validate_markdown(content: &str) -> Result<(), KnowledgeError> {
    if content.as_bytes().contains(&0) {
        Err(KnowledgeError::InvalidMarkdown)
    } else {
        Ok(())
    }
}

fn written_token(outcome: EditOutcome, path: &Path) -> Result<FileToken, KnowledgeError> {
    match outcome {
        EditOutcome::Written(version) => Ok(FileToken {
            revision: Some(version.revision),
            digest: version.digest,
        }),
        EditOutcome::Conflict(_) => Err(KnowledgeError::Conflict(path_string(path))),
    }
}

fn page_title(path: &Path, content: &str) -> String {
    content
        .lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|title| !title.is_empty())
        .map_or_else(
            || {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            },
            ToOwned::to_owned,
        )
}

fn markdown_links(content: &str) -> Vec<MarkdownLink> {
    let mut links = Vec::new();
    let mut offset = 0;
    while let Some(relative_start) = content[offset..].find("](") {
        let start = offset + relative_start + 2;
        let Some(relative_end) = content[start..].find(')') else {
            break;
        };
        let end = start + relative_end;
        let href = content[start..end].trim();
        let leading = content[start..end].len() - content[start..end].trim_start().len();
        links.push(MarkdownLink {
            href_range: start + leading..start + leading + href.len(),
            href: href.to_owned(),
        });
        offset = end + 1;
    }
    links
}

fn resolve_link(page: &Path, href: &str) -> Result<Option<PathBuf>, KnowledgeError> {
    let href = href
        .split(['#', '?'])
        .next()
        .unwrap_or("")
        .trim_matches(['<', '>']);
    if href.is_empty()
        || href.starts_with('/')
        || href
            .split('/')
            .next()
            .is_some_and(|first| first.contains(':'))
    {
        return Ok(None);
    }
    let mut parts = page
        .parent()
        .ok_or(KnowledgeError::UnsafePath)?
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_owned()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for component in Path::new(href).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => parts.push(value.to_owned()),
            Component::ParentDir if parts.len() > 1 => {
                parts.pop();
            }
            _ => return Err(KnowledgeError::UnsafePath),
        }
    }
    let target = parts.into_iter().collect::<PathBuf>();
    if !target.starts_with("knowledge") || target.extension().is_none_or(|value| value != "md") {
        return Err(KnowledgeError::UnsafePath);
    }
    Ok(Some(target))
}

fn rewrite_links(
    content: &str,
    source_page: &Path,
    destination_page: &Path,
    replacement: Option<(&Path, &Path)>,
) -> Result<String, KnowledgeError> {
    let replacements = replacement
        .map(|(from, to)| BTreeMap::from([(from.to_path_buf(), to.to_path_buf())]))
        .unwrap_or_default();
    rewrite_links_internal(
        content,
        source_page,
        destination_page,
        &replacements,
        source_page != destination_page,
    )
}

fn rewrite_link_map(
    content: &str,
    page: &Path,
    replacements: &BTreeMap<PathBuf, PathBuf>,
) -> Result<String, KnowledgeError> {
    rewrite_links_internal(content, page, page, replacements, false)
}

fn rewrite_links_internal(
    content: &str,
    source_page: &Path,
    destination_page: &Path,
    replacements: &BTreeMap<PathBuf, PathBuf>,
    moving: bool,
) -> Result<String, KnowledgeError> {
    let mut output = content.to_owned();
    let mut edits = Vec::new();
    for link in markdown_links(content) {
        let Some(target) = resolve_link(source_page, &link.href)? else {
            continue;
        };
        let replacement = replacements.get(&target).unwrap_or(&target);
        if moving || replacement != &target {
            edits.push((
                link.href_range,
                relative_link(destination_page, replacement)?,
            ));
        }
    }
    for (range, replacement) in edits.into_iter().rev() {
        output.replace_range(range, &replacement);
    }
    Ok(output)
}

fn relative_link(from_page: &Path, target: &Path) -> Result<String, KnowledgeError> {
    let from = from_page
        .parent()
        .ok_or(KnowledgeError::UnsafePath)?
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_owned()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let target = target
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_owned()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let common = from
        .iter()
        .zip(&target)
        .take_while(|(left, right)| left == right)
        .count();
    let mut relative = PathBuf::new();
    for _ in common..from.len() {
        relative.push("..");
    }
    for component in &target[common..] {
        relative.push(component);
    }
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn collect_pages(root: &Path, relative: &Path) -> Result<Vec<PathBuf>, KnowledgeError> {
    let path = root.join(relative);
    let mut pages = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.file_type()?;
        if metadata.is_symlink() {
            return Err(KnowledgeError::UnsafePath);
        }
        let child = relative.join(entry.file_name());
        if metadata.is_dir() {
            pages.extend(collect_pages(root, &child)?);
        } else if child.extension().is_some_and(|extension| extension == "md") {
            pages.push(child);
        }
    }
    pages.sort();
    Ok(pages)
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use keith_retrieval::{RankWeights, RetrievalLimits};
    use keith_workspace::PersonalWorkspaceLimits;
    use tempfile::tempdir;

    use super::*;

    fn service(root: &Path) -> KnowledgeService {
        let workspace = PersonalWorkspace::open(
            root.join("workspace"),
            PersonalWorkspaceLimits::default(),
            UtcTimestamp::UNIX_EPOCH,
        )
        .unwrap();
        let retrieval = Arc::new(
            RetrievalService::open(
                root.join("index"),
                RetrievalLimits::default(),
                RankWeights::default(),
                None,
            )
            .unwrap(),
        );
        KnowledgeService::new(workspace, retrieval, ProfileId::new())
    }

    #[test]
    fn crud_links_backlinks_orphans_rename_search_and_rebuild_are_consistent() {
        let directory = tempdir().unwrap();
        let service = service(directory.path());
        let alpha = service
            .create(
                "alpha.md",
                "# Alpha\nSee [Beta](beta.md).",
                UtcTimestamp::UNIX_EPOCH,
            )
            .unwrap();
        service
            .create(
                "beta.md",
                "# Beta\nHybrid retrieval notes. Return to [Alpha](alpha.md).",
                UtcTimestamp::from_unix_millis(1),
            )
            .unwrap();
        service
            .create(
                "orphan.md",
                "# Orphan\nStandalone page.",
                UtcTimestamp::from_unix_millis(2),
            )
            .unwrap();
        assert_eq!(
            service
                .backlinks("beta.md", UtcTimestamp::from_unix_millis(3))
                .unwrap(),
            vec!["knowledge/alpha.md"]
        );
        assert_eq!(
            service.orphans(UtcTimestamp::from_unix_millis(3)).unwrap(),
            vec!["knowledge/orphan.md"]
        );
        let updated = service
            .update(
                "alpha.md",
                &alpha.token,
                "# Alpha\nSee [Beta](beta.md) and implementation details.",
                UtcTimestamp::from_unix_millis(4),
            )
            .unwrap();
        assert!(updated.content.contains("implementation"));
        let moved = service
            .rename(
                "beta.md",
                "archive/beta.md",
                UtcTimestamp::from_unix_millis(5),
            )
            .unwrap();
        assert!(moved.content.contains("../alpha.md"));
        let alpha = service
            .inspect("alpha.md", UtcTimestamp::from_unix_millis(6))
            .unwrap();
        assert!(alpha.content.contains("archive/beta.md"));
        let graph = service.graph(UtcTimestamp::from_unix_millis(6)).unwrap();
        assert!(graph.broken.is_empty());
        assert!(
            graph
                .edges
                .get("knowledge/alpha.md")
                .unwrap()
                .contains("knowledge/archive/beta.md")
        );
        service
            .rebuild_index(UtcTimestamp::from_unix_millis(7))
            .unwrap();
        let results = service.search("hybrid retrieval", 5).unwrap();
        assert_eq!(results[0].source_path, "knowledge/archive/beta.md");
        service
            .delete(
                "archive/beta.md",
                &moved.token,
                UtcTimestamp::from_unix_millis(8),
            )
            .unwrap();
        assert!(service.search("hybrid retrieval", 5).unwrap().is_empty());
    }

    #[test]
    fn broken_links_repair_and_human_conflicts_are_visible() {
        let directory = tempdir().unwrap();
        let service = service(directory.path());
        let source = service
            .create(
                "source.md",
                "# Source\nRead [Old](missing.md).",
                UtcTimestamp::UNIX_EPOCH,
            )
            .unwrap();
        service
            .create(
                "target.md",
                "# Target\nReplacement material.",
                UtcTimestamp::from_unix_millis(1),
            )
            .unwrap();
        let graph = service.graph(UtcTimestamp::from_unix_millis(2)).unwrap();
        assert_eq!(graph.broken.len(), 1);
        let changed = service
            .repair_links(
                &BTreeMap::from([("missing.md".into(), "target.md".into())]),
                UtcTimestamp::from_unix_millis(3),
            )
            .unwrap();
        assert_eq!(changed, 1);
        assert!(
            service
                .graph(UtcTimestamp::from_unix_millis(4))
                .unwrap()
                .broken
                .is_empty()
        );
        fs::write(
            service.workspace.layout().root.join("knowledge/source.md"),
            "# Human edit\nDo not overwrite.",
        )
        .unwrap();
        assert!(matches!(
            service.update(
                "source.md",
                &source.token,
                "# Agent stale edit",
                UtcTimestamp::from_unix_millis(5)
            ),
            Err(KnowledgeError::Conflict(_))
        ));
        assert!(
            fs::read_to_string(service.workspace.layout().root.join("knowledge/source.md"))
                .unwrap()
                .contains("Human edit")
        );
    }
}
