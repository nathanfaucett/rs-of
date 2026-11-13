#![cfg(feature = "std")]

use idp_service::repo::{PrivateKeyKeyringRepo, PrivateKeyRepo};
use key::DerivationPath;

#[cfg(unix)]
#[test]
fn local_private_keys_reject_public_or_symlink_directories() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let root = std::env::temp_dir().join(format!(
        "unsafe-local-private-{}",
        of_model::idp::model::Id::now_v7()
    ));
    std::fs::create_dir(&root).expect("fixture directory");
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755))
        .expect("public fixture");
    assert!(PrivateKeyKeyringRepo::new_local(&root).is_err());
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
        .expect("private fixture");
    let link = root.with_extension("link");
    symlink(&root, &link).expect("symlink fixture");
    assert!(PrivateKeyKeyringRepo::new_local(&link).is_err());
    std::fs::remove_file(link).expect("remove fixture link");
    std::fs::remove_dir(root).expect("remove fixture directory");
}

#[test]
fn local_private_keys_persist_without_an_os_credential_store() {
    let root = std::env::temp_dir().join(format!(
        "local-private-{}",
        of_model::idp::model::Id::now_v7()
    ));
    let repo = PrivateKeyKeyringRepo::new_local(&root).expect("file-backed local keys");
    let key = repo.ensure_master_key("local").expect("create local key");
    drop(repo);
    let reopened = PrivateKeyKeyringRepo::new_local(&root).expect("reopen local keys");
    assert_eq!(
        reopened
            .load("local", &DerivationPath::default())
            .expect("read key")
            .expect("retained key")
            .to_xprv_string(),
        key.to_xprv_string()
    );
    assert!(
        reopened
            .load("other", &DerivationPath::default())
            .expect("separate namespace")
            .is_none()
    );
    reopened
        .delete("local", &DerivationPath::default())
        .expect("delete local key");
    assert!(
        reopened
            .load("local", &DerivationPath::default())
            .expect("deleted key")
            .is_none()
    );
    std::fs::remove_dir_all(root).expect("remove local keys");
}
