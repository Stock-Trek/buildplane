use buildplane_errors::{BpError, BpResult, ValidationError};
use buildplane_git::git_host::GitHost;
use buildplane_messages::sqs_message::SqsRefType;
use std::fmt::Display;

const SOURCE_PREFIX: &str = "source/";
const ARTIFACT_PREFIX: &str = "artifact/";

pub struct S3Keys;

impl S3Keys {
    pub fn ref_key<RepoId, Remote>(
        git_remote: &Remote,
        repo_id: RepoId,
        ref_name: &str,
        ref_type: SqsRefType,
        commit_hash: &str,
    ) -> BpResult<String>
    where
        RepoId: Display,
        Remote: GitHost<RepoId>,
    {
        let repo = Self::sanitize_path(&repo_id.to_string())?;
        let ref_name = Self::sanitize_path(ref_name)?;
        let commit_hash = Self::sanitize_path(commit_hash)?;
        let ref_type_name = match ref_type {
            SqsRefType::Branch => "heads",
            SqsRefType::Tag => "tags",
        };
        Ok(format!(
            "{}/{}/{}/{}/{}/{}/",
            git_remote.provider(),
            git_remote.host(),
            repo,
            ref_type_name,
            ref_name,
            commit_hash
        ))
    }

    pub fn source_prefix<RepoId, Remote>(
        git_remote: &Remote,
        repo_id: RepoId,
        ref_name: &str,
        ref_type: SqsRefType,
        commit_hash: &str,
    ) -> BpResult<String>
    where
        RepoId: Display,
        Remote: GitHost<RepoId>,
    {
        Ok(format!(
            "{}{SOURCE_PREFIX}",
            Self::ref_key(git_remote, repo_id, ref_name, ref_type, commit_hash)?
        ))
    }

    pub fn source_key<RepoId, Remote>(
        git_remote: &Remote,
        repo_id: RepoId,
        ref_name: &str,
        ref_type: SqsRefType,
        commit_hash: &str,
    ) -> BpResult<String>
    where
        RepoId: Display,
        Remote: GitHost<RepoId>,
    {
        Ok(format!(
            "{}algorithms.zip",
            Self::source_prefix(git_remote, repo_id, ref_name, ref_type, commit_hash)?
        ))
    }

    pub fn artifact_prefix<RepoId, Remote>(
        git_remote: &Remote,
        repo_id: RepoId,
        ref_name: &str,
        ref_type: SqsRefType,
        commit_hash: &str,
    ) -> BpResult<String>
    where
        RepoId: Display,
        Remote: GitHost<RepoId>,
    {
        Ok(format!(
            "{}{ARTIFACT_PREFIX}",
            Self::ref_key(git_remote, repo_id, ref_name, ref_type, commit_hash)?
        ))
    }

    pub fn artifact_key<RepoId, Remote>(
        git_remote: &Remote,
        repo_id: RepoId,
        ref_name: &str,
        ref_type: SqsRefType,
        commit_hash: &str,
    ) -> BpResult<String>
    where
        RepoId: Display,
        Remote: GitHost<RepoId>,
    {
        Ok(format!(
            "{}runner.cwasm",
            Self::artifact_prefix(git_remote, repo_id, ref_name, ref_type, commit_hash)?
        ))
    }

    pub fn compile_output_key<RepoId, Remote>(
        git_remote: &Remote,
        repo_id: RepoId,
        ref_name: &str,
        ref_type: SqsRefType,
        commit_hash: &str,
    ) -> BpResult<String>
    where
        RepoId: Display,
        Remote: GitHost<RepoId>,
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
    use buildplane_git::git_host::GitFile;
    use buildplane_time::deadline::Deadline;

    struct FakeRemote;

    impl GitHost<u64> for FakeRemote {
        fn provider(&self) -> &'static str {
            "github"
        }

        fn host(&self) -> &str {
            "github.com"
        }

        fn parse_repo_id(&self, repo_id: &str) -> BpResult<u64> {
            Ok(repo_id.parse().expect("test repo id"))
        }

        async fn repo_ids(&self, _deadline: Deadline) -> BpResult<Vec<u64>> {
            Ok(Vec::new())
        }

        async fn repo_default_branch(
            &self,
            _repo_id: u64,
            _deadline: Deadline,
        ) -> BpResult<String> {
            Ok("main".to_string())
        }

        async fn ref_commit(
            &self,
            _repo_id: u64,
            _ref_name: &str,
            _deadline: Deadline,
        ) -> BpResult<String> {
            Ok("abc123".to_string())
        }

        async fn directory_files(
            &self,
            _repo_id: u64,
            _ref_name: &str,
            _directory: &str,
            _deadline: Deadline,
        ) -> BpResult<Vec<GitFile>> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn source_key_omits_installation_id() {
        assert_eq!(
            S3Keys::source_key(&FakeRemote, 7u64, "main", SqsRefType::Branch, "abc123").unwrap(),
            "github/github.com/7/heads/main/abc123/source/algorithms.zip"
        );
    }

    #[test]
    fn artifact_key_omits_installation_id() {
        assert_eq!(
            S3Keys::artifact_key(&FakeRemote, 7u64, "v1", SqsRefType::Tag, "abc123").unwrap(),
            "github/github.com/7/tags/v1/abc123/artifact/runner.cwasm"
        );
    }
}
