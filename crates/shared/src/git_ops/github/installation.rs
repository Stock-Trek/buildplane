use crate::{
    deadline::Deadline,
    errors::{
        error::{RsSharedError, RsSharedResult},
        github::RsGitHubError,
        invalid_message::RsInvalidMessageError,
        timeout::RsTimeoutError,
    },
    git_ops::git_remote::{GitFile, GitHost},
    timeouts::Timeouts,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use octocrab::{Octocrab, models::InstallationId};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Arc};

const PROVIDER: &str = "github";
const REPOS_PER_PAGE: u32 = 100;

pub const DEFAULT_MAX_DIRECTORY_DEPTH: usize = 8;
pub const DEFAULT_MAX_DIRECTORY_FILES: usize = 100;
pub const DEFAULT_MAX_DIRECTORY_BYTES: u64 = 1024 * 1024;

/// Limits applied while collecting a directory tree.
///
/// Directory entries are loaded into memory before being written to the source
/// archive, so these limits keep a pathological repository from exhausting the
/// Lambda's memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectoryLimits {
    pub max_depth: usize,
    pub max_files: usize,
    pub max_total_bytes: u64,
}

impl Default for DirectoryLimits {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_MAX_DIRECTORY_DEPTH,
            max_files: DEFAULT_MAX_DIRECTORY_FILES,
            max_total_bytes: DEFAULT_MAX_DIRECTORY_BYTES,
        }
    }
}

#[derive(Debug, Serialize)]
struct RepositoriesParams {
    per_page: u32,
    page: u32,
}

#[derive(Debug, Serialize)]
struct CommitsParams<'a> {
    sha: &'a str,
    per_page: u32,
}

#[derive(Debug, Serialize)]
struct TreeParams {
    recursive: u8,
}

#[derive(Debug, Deserialize)]
struct GitHubCommit {
    sha: String,
}

#[derive(Debug, Deserialize)]
struct GitHubTree {
    #[serde(default)]
    tree: Vec<GitHubTreeEntry>,
    #[serde(default)]
    truncated: bool,
}

#[derive(Debug, Deserialize)]
struct GitHubTreeEntry {
    path: String,
    #[serde(default)]
    mode: String,
    #[serde(rename = "type")]
    kind: String,
    sha: String,
    #[serde(default)]
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GitHubBlob {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    encoding: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InstallationRepositoriesResponse {
    repositories: Vec<GitHubRepo>,
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubRepo {
    id: u64,
    default_branch: String,
}

#[derive(Clone)]
pub struct GitHubInstallation {
    client: Arc<Octocrab>,
    host: String,
    installation_id: u64,
    timeouts: Timeouts,
    directory_limits: DirectoryLimits,
}

impl GitHost<u64, u64> for GitHubInstallation {
    fn provider(&self) -> &'static str {
        PROVIDER
    }

    fn host(&self) -> &str {
        &self.host
    }

    fn installation_id(&self) -> u64 {
        self.installation_id
    }

    fn parse_repo_id(&self, repo_id: &str) -> RsSharedResult<u64> {
        repo_id.parse().map_err(|error| {
            RsSharedError::InvalidMessage(RsInvalidMessageError::GitHubRepositoryId {
                repo_id: repo_id.to_owned(),
                source: error,
            })
        })
    }

    async fn repo_ids(&self, deadline: Deadline) -> RsSharedResult<Vec<u64>> {
        let timeout = self.timeouts.operation_timeout(deadline)?;
        let repos = tokio::time::timeout(timeout, self.repos())
            .await
            .map_err(|_| {
                RsSharedError::Timeout(RsTimeoutError::Operation {
                    operation: "GitHub list repositories",
                })
            })??;
        Ok(repos.into_iter().map(|repo| repo.id).collect())
    }

    async fn repo_default_branch(
        &self,
        repo_id: u64,
        deadline: Deadline,
    ) -> RsSharedResult<String> {
        let timeout = self.timeouts.operation_timeout(deadline)?;
        let repo = tokio::time::timeout(timeout, self.repo(repo_id))
            .await
            .map_err(|_| {
                RsSharedError::Timeout(RsTimeoutError::Operation {
                    operation: "GitHub get repository",
                })
            })?;
        repo.map(|r| r.default_branch)
    }

    async fn ref_commit(
        &self,
        repo_id: u64,
        ref_name: &str,
        deadline: Deadline,
    ) -> RsSharedResult<String> {
        let timeout = self.timeouts.operation_timeout(deadline)?;
        let params = CommitsParams {
            sha: ref_name,
            per_page: 1,
        };
        let commits = tokio::time::timeout(
            timeout,
            self.client.get::<Vec<GitHubCommit>, _, _>(
                format!("/repositories/{repo_id}/commits"),
                Some(&params),
            ),
        )
        .await
        .map_err(|_| {
            RsSharedError::Timeout(RsTimeoutError::Operation {
                operation: "GitHub get commit",
            })
        })?
        .map_err(|error| {
            RsSharedError::GitHub(RsGitHubError::GetCommit {
                repo_id,
                ref_name: ref_name.to_owned(),
                source: error,
            })
        })?;
        commits
            .into_iter()
            .next()
            .map(|commit| commit.sha)
            .ok_or_else(|| {
                RsSharedError::GitHub(RsGitHubError::CommitNotFound {
                    repo_id,
                    ref_name: ref_name.to_owned(),
                })
            })
    }

    async fn directory_files(
        &self,
        repo_id: u64,
        ref_name: &str,
        directory: &str,
        deadline: Deadline,
    ) -> RsSharedResult<Vec<GitFile>> {
        let timeout = self.timeouts.operation_timeout(deadline)?;
        tokio::time::timeout(
            timeout,
            self.collect_directory_files(repo_id, ref_name, directory),
        )
        .await
        .map_err(|_| {
            RsSharedError::Timeout(RsTimeoutError::Operation {
                operation: "GitHub get repository contents",
            })
        })?
    }
}

impl GitHubInstallation {
    pub fn new(
        app: &Octocrab,
        host: &str,
        installation_id: u64,
        timeouts: Timeouts,
    ) -> RsSharedResult<Self> {
        Self::with_limits(
            app,
            host,
            installation_id,
            timeouts,
            DirectoryLimits::default(),
        )
    }

    pub fn with_limits(
        app: &Octocrab,
        host: &str,
        installation_id: u64,
        timeouts: Timeouts,
        directory_limits: DirectoryLimits,
    ) -> RsSharedResult<Self> {
        let client = app
            .installation(InstallationId::from(installation_id))
            .map_err(|error| {
                RsSharedError::GitHub(RsGitHubError::Installation {
                    installation_id,
                    source: error,
                })
            })?;
        Ok(Self {
            client: Arc::new(client),
            host: host.to_owned(),
            installation_id,
            timeouts,
            directory_limits,
        })
    }
}

// helpers
impl GitHubInstallation {
    async fn collect_directory_files(
        &self,
        repo_id: u64,
        ref_name: &str,
        directory: &str,
    ) -> RsSharedResult<Vec<GitFile>> {
        let directory = directory.trim_matches('/');
        if !directory.is_empty() {
            Self::validate_relative_path(repo_id, directory, "")?;
        }

        // The Contents API silently caps directory listings at 1,000 entries.
        // The Git Trees API returns the complete tree (and reports when GitHub
        // had to truncate it), so use that for directory discovery instead.
        let tree = self.tree(repo_id, ref_name).await?;
        if tree.truncated {
            return Err(RsSharedError::GitHub(RsGitHubError::TreeTruncated {
                repo_id,
                ref_name: ref_name.to_owned(),
            }));
        }

        if !directory.is_empty()
            && !tree
                .tree
                .iter()
                .any(|entry| entry.path == directory && entry.kind == "tree")
        {
            return Err(RsSharedError::GitHub(RsGitHubError::DirectoryNotFound {
                repo_id,
                path: directory.to_owned(),
                ref_name: ref_name.to_owned(),
            }));
        }

        let prefix = if directory.is_empty() {
            String::new()
        } else {
            format!("{directory}/")
        };
        let mut files = Vec::new();
        let mut total_bytes = 0_u64;
        for entry in tree.tree {
            let Some(relative) = entry.path.strip_prefix(&prefix) else {
                continue;
            };
            if relative.is_empty() {
                continue;
            }

            let depth = relative.matches('/').count();
            if depth > self.directory_limits.max_depth {
                return Err(RsSharedError::GitHub(
                    RsGitHubError::DirectoryDepthExceeded {
                        repo_id,
                        path: entry.path,
                        max_depth: self.directory_limits.max_depth,
                    },
                ));
            }
            Self::validate_relative_path(repo_id, relative, directory)?;

            match entry.kind.as_str() {
                "tree" => {}
                "blob" => {
                    // Symlinks are blobs with mode 120000; like submodules they
                    // are not part of the source archive.
                    if entry.mode == "120000" {
                        continue;
                    }
                    if files.len() >= self.directory_limits.max_files {
                        return Err(RsSharedError::GitHub(RsGitHubError::TooManyFiles {
                            repo_id,
                            directory: directory.to_owned(),
                            max_files: self.directory_limits.max_files,
                        }));
                    }
                    if let Some(size) = entry.size
                        && total_bytes.saturating_add(size) > self.directory_limits.max_total_bytes
                    {
                        return Err(RsSharedError::GitHub(RsGitHubError::DirectoryTooLarge {
                            repo_id,
                            directory: directory.to_owned(),
                            max_bytes: self.directory_limits.max_total_bytes,
                        }));
                    }
                    let contents = self.blob(repo_id, &entry.sha).await?;
                    total_bytes = total_bytes.saturating_add(contents.len() as u64);
                    if total_bytes > self.directory_limits.max_total_bytes {
                        return Err(RsSharedError::GitHub(RsGitHubError::DirectoryTooLarge {
                            repo_id,
                            directory: directory.to_owned(),
                            max_bytes: self.directory_limits.max_total_bytes,
                        }));
                    }
                    files.push(GitFile {
                        path: relative.to_owned(),
                        contents,
                    });
                }
                "commit" => {}
                kind => {
                    return Err(RsSharedError::GitHub(
                        RsGitHubError::UnsupportedContentType {
                            repo_id,
                            path: entry.path,
                            content_type: kind.to_owned(),
                        },
                    ));
                }
            }
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(files)
    }

    async fn tree(&self, repo_id: u64, ref_name: &str) -> RsSharedResult<GitHubTree> {
        let params = TreeParams { recursive: 1 };
        self.client
            .get::<GitHubTree, _, _>(
                format!(
                    "/repositories/{repo_id}/git/trees/{}",
                    Self::encode_path(ref_name)
                ),
                Some(&params),
            )
            .await
            .map_err(|source| {
                RsSharedError::GitHub(RsGitHubError::GetTree {
                    repo_id,
                    ref_name: ref_name.to_owned(),
                    source: Box::new(source),
                })
            })
    }

    async fn blob(&self, repo_id: u64, sha: &str) -> RsSharedResult<Vec<u8>> {
        let blob = self
            .client
            .get::<GitHubBlob, _, _>(
                format!("/repositories/{repo_id}/git/blobs/{sha}"),
                None::<&()>,
            )
            .await
            .map_err(|source| {
                RsSharedError::GitHub(RsGitHubError::GetBlob {
                    repo_id,
                    sha: sha.to_owned(),
                    source: Box::new(source),
                })
            })?;
        Self::decode_content(
            repo_id,
            sha,
            blob.content.as_deref(),
            blob.encoding.as_deref(),
        )?
        .ok_or_else(|| {
            RsSharedError::GitHub(RsGitHubError::MissingContent {
                repo_id,
                path: sha.to_owned(),
            })
        })
    }

    fn decode_content(
        repo_id: u64,
        path: &str,
        content: Option<&str>,
        encoding: Option<&str>,
    ) -> RsSharedResult<Option<Vec<u8>>> {
        let Some(content) = content else {
            return Ok(None);
        };
        match encoding {
            Some("base64") => {
                let compact = content
                    .chars()
                    .filter(|character| !character.is_ascii_whitespace())
                    .collect::<String>();
                BASE64.decode(compact).map(Some).map_err(|source| {
                    RsSharedError::GitHub(RsGitHubError::DecodeContent {
                        repo_id,
                        path: path.to_owned(),
                        encoding: "base64",
                        source,
                    })
                })
            }
            Some("utf8") => Ok(Some(content.as_bytes().to_vec())),
            Some("none") | None if content.is_empty() => Ok(None),
            Some(encoding) => Err(RsSharedError::GitHub(
                RsGitHubError::UnsupportedContentEncoding {
                    repo_id,
                    path: path.to_owned(),
                    encoding: encoding.to_owned(),
                },
            )),
            None => Ok(Some(content.as_bytes().to_vec())),
        }
    }

    fn encode_path(path: &str) -> String {
        path.split('/')
            .map(|segment| {
                let mut encoded = String::new();
                for byte in segment.bytes() {
                    match byte {
                        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                            encoded.push(char::from(byte))
                        }
                        byte => encoded.push_str(&format!("%{byte:02X}")),
                    }
                }
                encoded
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    fn validate_relative_path(repo_id: u64, path: &str, directory: &str) -> RsSharedResult<()> {
        let path = Path::new(path);
        if path.as_os_str().is_empty()
            || path.to_string_lossy().contains('\\')
            || path
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(RsSharedError::GitHub(RsGitHubError::InvalidContentPath {
                repo_id,
                path: path.display().to_string(),
                directory: directory.to_owned(),
            }));
        }
        Ok(())
    }

    async fn repos(&self) -> RsSharedResult<Vec<GitHubRepo>> {
        let mut repos = Vec::new();
        let mut page = 1;
        loop {
            let params = RepositoriesParams {
                per_page: REPOS_PER_PAGE,
                page,
            };
            let response = self
                .client
                .get::<InstallationRepositoriesResponse, _, _>(
                    "/installation/repositories",
                    Some(&params),
                )
                .await
                .map_err(|error| RsSharedError::GitHub(RsGitHubError::ListRepositories(error)))?;
            let received = response.repositories.len();
            repos.extend(response.repositories);
            if received < REPOS_PER_PAGE as usize {
                break;
            }
            page += 1;
        }
        Ok(repos)
    }

    async fn repo(&self, repo_id: u64) -> RsSharedResult<GitHubRepo> {
        self.client
            .get::<GitHubRepo, _, _>(format!("/repositories/{repo_id}"), None::<&()>)
            .await
            .map_err(|error| {
                if Self::is_not_found(&error) {
                    RsSharedError::GitHub(RsGitHubError::RepositoryNotFound {
                        repo_id,
                        source: error,
                    })
                } else {
                    RsSharedError::GitHub(RsGitHubError::GetRepository {
                        repo_id,
                        source: error,
                    })
                }
            })
    }

    fn is_not_found(error: &octocrab::Error) -> bool {
        matches!(
            error,
            octocrab::Error::GitHub { source, .. } if source.status_code.as_u16() == 404
        )
    }
}
