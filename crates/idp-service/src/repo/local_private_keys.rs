use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use key::{DerivationPath, DerivedKey};
use sha2::{Digest, Sha256};

use crate::repo::{RepoError, RepoResult};

pub struct LocalPrivateKeys(PathBuf);

impl LocalPrivateKeys {
    pub fn new(root: &Path) -> RepoResult<Self> {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(root).map_err(error)?;
        require_private(root, true)?;
        Ok(Self(root.to_owned()))
    }

    fn path(&self, namespace: &str, derivation: &DerivationPath) -> PathBuf {
        let digest = Sha256::digest(format!("{namespace}\0{derivation}").as_bytes());
        self.0.join(format!("{digest:x}.key"))
    }

    pub fn load(
        &self,
        namespace: &str,
        derivation: &DerivationPath,
    ) -> RepoResult<Option<DerivedKey>> {
        let path = self.path(namespace, derivation);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(error(source)),
            Ok(_) => require_private(&path, false)?,
        }
        let secret = fs::read_to_string(path).map_err(error)?;
        Ok(Some(DerivedKey::from_xprv(secret, derivation.clone())?))
    }

    pub fn store(&self, namespace: &str, key: &DerivedKey) -> RepoResult<()> {
        require_private(&self.0, true)?;
        let path = self.path(namespace, key.derivation_path());
        let temporary = path.with_extension(format!("{}.tmp", of_model::idp::model::Id::now_v7()));
        let mut options = fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let result = (|| {
            let mut file = options.open(&temporary)?;
            file.write_all(key.to_xprv_string().as_bytes())?;
            file.sync_all()?;
            fs::rename(&temporary, &path)?;
            fs::File::open(&self.0)?.sync_all()
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result.map_err(error)
    }

    pub fn delete(&self, namespace: &str, derivation: &DerivationPath) -> RepoResult<()> {
        match fs::remove_file(self.path(namespace, derivation)) {
            Ok(()) => fs::File::open(&self.0)
                .and_then(|directory| directory.sync_all())
                .map_err(error),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(error(source)),
        }
    }
}

fn require_private(path: &Path, directory: bool) -> RepoResult<()> {
    let metadata = fs::symlink_metadata(path).map_err(error)?;
    if metadata.is_symlink()
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        return Err(RepoError::InvalidInput(
            "local key storage must use regular files and directories".into(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(RepoError::InvalidInput(
                "local key storage must deny group and other access".into(),
            ));
        }
    }
    Ok(())
}

fn error(source: io::Error) -> RepoError {
    RepoError::InvalidInput(source.to_string())
}
