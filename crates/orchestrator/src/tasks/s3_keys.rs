use buildplane_errors::BpError;
use buildplane_errors::BpResult;
use buildplane_errors::ValidationError;
use buildplane_git::git_host::GitHost;
use buildplane_messages::sqs_message::SqsRefType;
use std::fmt::Display;

const SOURCE_PREFIX: &str = "source/";
const ARTIFACT_PREFIX: &str = "artifact/";

pub struct S3Keys;

impl S3Keys {
    pub fn host_prefix(provider: &str, host: &str) -> String {
        format!("{provider}/{host}/")
    }

    /// Rewrites a repository object key so it points at `destination_installation_id`.
    ///
    /// Keys are laid out as
    /// `{provider}/{host}/{installation_id}/{repo_id}/...`, so a repository
    /// transfer only changes the installation segment. Returns `None` when the
    /// key belongs to a different provider, host, or repository.
    pub fn transfer_key(
        key: &str,
        provider: &str,
        host: &str,
        repo_id: &str,
        destination_installation_id: &str,
    ) -> Option<String> {
        let mut segments = key.splitn(5, '/');
        let key_provider = segments.next()?;
        let key_host = segments.next()?;
        let _key_installation_id = segments.next()?;
        let key_repo_id = segments.next()?;
        let remainder = segments.next()?;
        if key_provider != provider || key_host != host || key_repo_id != repo_id {
            return None;
        }
        Some(format!(
            "{provider}/{host}/{destination_installation_id}/{repo_id}/{remainder}"
        ))
    }
    pub fn ref_key<InstallationId, RepoId, Remote>(
        git_remote: &Remote,
        repo_id: RepoId,
        ref_name: &str,
        ref_type: SqsRefType,
        commit_hash: &str,
    ) -> BpResult<String>
    where
        InstallationId: Display,
        RepoId: Display,
        Remote: GitHost<InstallationId, RepoId>,
    {
        let repo = Self::sanitize_path(&repo_id.to_string())?;
        let ref_name = Self::sanitize_path(ref_name)?;
        let commit_hash = Self::sanitize_path(commit_hash)?;
        let ref_type_name = match ref_type {
            SqsRefType::Branch => "heads",
            SqsRefType::Tag => "tags",
        };
        Ok(format!(
            "{}/{}/{}/{}/{}/{}/{}/",
            git_remote.provider(),
            git_remote.host(),
            git_remote.installation_id(),
            repo,
            ref_type_name,
            ref_name,
            commit_hash
        ))
    }

    pub fn source_prefix<InstallationId, RepoId, Remote>(
        git_remote: &Remote,
        repo_id: RepoId,
        ref_name: &str,
        ref_type: SqsRefType,
        commit_hash: &str,
    ) -> BpResult<String>
    where
        InstallationId: Display,
        RepoId: Display,
        Remote: GitHost<InstallationId, RepoId>,
    {
        Ok(format!(
            "{}{SOURCE_PREFIX}",
            Self::ref_key(git_remote, repo_id, ref_name, ref_type, commit_hash)?
        ))
    }

    pub fn source_key<InstallationId, RepoId, Remote>(
        git_remote: &Remote,
        repo_id: RepoId,
        ref_name: &str,
        ref_type: SqsRefType,
        commit_hash: &str,
    ) -> BpResult<String>
    where
        InstallationId: Display,
        RepoId: Display,
        Remote: GitHost<InstallationId, RepoId>,
    {
        Ok(format!(
            "{}algorithms.zip",
            Self::source_prefix(git_remote, repo_id, ref_name, ref_type, commit_hash)?
        ))
    }

    pub fn artifact_prefix<InstallationId, RepoId, Remote>(
        git_remote: &Remote,
        repo_id: RepoId,
        ref_name: &str,
        ref_type: SqsRefType,
        commit_hash: &str,
    ) -> BpResult<String>
    where
        InstallationId: Display,
        RepoId: Display,
        Remote: GitHost<InstallationId, RepoId>,
    {
        Ok(format!(
            "{}{ARTIFACT_PREFIX}",
            Self::ref_key(git_remote, repo_id, ref_name, ref_type, commit_hash)?
        ))
    }

    pub fn artifact_key<InstallationId, RepoId, Remote>(
        git_remote: &Remote,
        repo_id: RepoId,
        ref_name: &str,
        ref_type: SqsRefType,
        commit_hash: &str,
    ) -> BpResult<String>
    where
        InstallationId: Display,
        RepoId: Display,
        Remote: GitHost<InstallationId, RepoId>,
    {
        Ok(format!(
            "{}runner.cwasm",
            Self::artifact_prefix(git_remote, repo_id, ref_name, ref_type, commit_hash)?
        ))
    }

    pub fn compile_output_key<InstallationId, RepoId, Remote>(
        git_remote: &Remote,
        repo_id: RepoId,
        ref_name: &str,
        ref_type: SqsRefType,
        commit_hash: &str,
    ) -> BpResult<String>
    where
        InstallationId: Display,
        RepoId: Display,
        Remote: GitHost<InstallationId, RepoId>,
    {
        Ok(format!(
            "{}compile-output.txt",
            Self::artifact_prefix(git_remote, repo_id, ref_name, ref_type, commit_hash)?
        ))
    }
}

impl S3Keys {
    fn sanitize_path(value: &str) -> BpResult<String> {
        if Self::is_valid_ref_name(value) {
            Ok(value.to_string())
        } else {
            Err(BpError::Validation(ValidationError::InvalidPath {
                value: value.to_string(),
            }))?
        }
    }

    fn is_valid_ref_name(value: &str) -> bool {
        !value.is_empty()
            && value != "@"
            && !value.starts_with('/')
            && !value.ends_with('/')
            && !value.ends_with('.')
            && !value.contains("//")
            && !value.contains("..")
            && !value.contains("@{")
            && value.split('/').all(|segment| {
                !segment.is_empty() && !segment.starts_with('.') && !segment.ends_with(".lock")
            })
            && value.chars().all(|character| {
                !character.is_ascii_control()
                    && character != ' '
                    && !matches!(character, '~' | '^' | ':' | '?' | '*' | '[' | '\\')
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_key_rewrites_installation_segment() {
        let key = "github/github.com/42/7/heads/main/abc123/source/algorithms.zip";

        assert_eq!(
            S3Keys::transfer_key(key, "github", "github.com", "7", "99"),
            Some("github/github.com/99/7/heads/main/abc123/source/algorithms.zip".to_string())
        );
    }

    #[test]
    fn transfer_key_rejects_other_repositories() {
        let key = "github/github.com/42/7/heads/main/abc123/source/algorithms.zip";

        assert_eq!(
            S3Keys::transfer_key(key, "github", "github.com", "8", "99"),
            None
        );
    }

    #[test]
    fn transfer_key_rejects_other_hosts_and_providers() {
        let key = "github/github.com/42/7/heads/main/abc123/source/algorithms.zip";

        assert_eq!(
            S3Keys::transfer_key(key, "github", "ghe.example.com", "7", "99"),
            None
        );
        assert_eq!(
            S3Keys::transfer_key(key, "gitlab", "github.com", "7", "99"),
            None
        );
    }

    #[test]
    fn transfer_key_rejects_malformed_keys() {
        assert_eq!(
            S3Keys::transfer_key("github/github.com/42", "github", "github.com", "42", "99"),
            None
        );
    }
}
