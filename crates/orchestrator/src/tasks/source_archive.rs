use buildplane_shared::errors::BpError;
use buildplane_shared::errors::BpResult;
use buildplane_shared::git_ops::git_remote::GitFile;
use std::io::{Cursor, Write};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

const USER_GENERATED_PREFIX: &str = "crates/runner/src/user_generated/";

pub struct SourceArchive;

impl SourceArchive {
    pub fn create(files: &[GitFile]) -> BpResult<Vec<u8>> {
        let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for file in files {
            archive
                .start_file(format!("{USER_GENERATED_PREFIX}{}", file.path), options)
                .map_err(BpError::from)?;
            archive.write_all(&file.contents).map_err(BpError::from)?;
        }
        Ok(archive.finish().map_err(BpError::from)?.into_inner())
    }
}
