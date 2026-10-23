use std::{
    fs, io,
    path::{Component, Path},
};

use crate::AppConfig;

impl AppConfig {
    /// Existing-only evidence for acknowledged initialization; never repairs runtime roots.
    pub fn require_provisioned_storage(&self) -> io::Result<()> {
        let root = Path::new(&self.data_dir);
        if root.as_os_str().is_empty()
            || root
                .components()
                .any(|part| matches!(part, Component::ParentDir))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "empty paths and parent components are not allowed",
            ));
        }
        let root = if root.is_absolute() {
            root.to_owned()
        } else {
            std::env::current_dir()?.join(root)
        };
        for directory in [
            root.clone(),
            root.join("databases"),
            root.join("filesystems"),
        ] {
            for ancestor in directory.ancestors() {
                if fs::symlink_metadata(ancestor)?.file_type().is_symlink() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "symlink ancestry is not allowed",
                    ));
                }
            }
            if !fs::symlink_metadata(directory)?.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "acknowledged Storage runtime roots require exact recovery",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::AppConfig;
    use std::fs;

    #[test]
    fn acknowledged_storage_requires_existing_roots_without_repair() {
        let root = std::env::temp_dir().join(format!(
            "storage-retained-{}",
            idp_model::model::Id::now_v7()
        ));
        let config = AppConfig {
            data_dir: root.to_string_lossy().into_owned(),
            ..Default::default()
        };
        assert!(config.require_provisioned_storage().is_err());
        assert!(!root.exists());
        fs::create_dir(&root).expect("root");
        fs::create_dir(root.join("databases")).expect("database root");
        assert!(config.require_provisioned_storage().is_err());
        assert!(!root.join("filesystems").exists());
        fs::write(root.join("filesystems"), b"not a directory").expect("malformed filesystem root");
        assert!(config.require_provisioned_storage().is_err());
        fs::remove_file(root.join("filesystems")).expect("remove malformed fixture");
        fs::create_dir(root.join("filesystems")).expect("filesystem root");
        config
            .require_provisioned_storage()
            .expect("acknowledged roots");
        #[cfg(unix)]
        {
            let linked = root.with_extension("symlink");
            std::os::unix::fs::symlink(&root, &linked).expect("symlink root");
            let linked_config = AppConfig {
                data_dir: linked.to_string_lossy().into_owned(),
                ..Default::default()
            };
            assert!(linked_config.require_provisioned_storage().is_err());
            fs::remove_file(linked).expect("remove symlink root");
            fs::remove_dir(root.join("filesystems")).expect("remove filesystem fixture");
            std::os::unix::fs::symlink(root.join("databases"), root.join("filesystems"))
                .expect("symlink child");
            assert!(config.require_provisioned_storage().is_err());
        }
        fs::remove_dir_all(root).expect("remove storage fixture");
    }
}
