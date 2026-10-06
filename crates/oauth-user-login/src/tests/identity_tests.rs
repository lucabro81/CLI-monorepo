#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use super::Identity;

#[test]
fn identity_follows_the_user_flag() {
    assert_eq!(Identity::from_user_flag(false), Identity::Service);
    assert_eq!(Identity::from_user_flag(true), Identity::User);
}

#[test]
fn each_identity_has_its_own_credentials_file_under_the_cli_dir() {
    let config_dir = Path::new("/home/user/.config");

    assert_eq!(
        Identity::Service.credentials_path(config_dir, "some-cli"),
        PathBuf::from("/home/user/.config/some-cli/credentials-service.json")
    );
    assert_eq!(
        Identity::User.credentials_path(config_dir, "some-cli"),
        PathBuf::from("/home/user/.config/some-cli/credentials-user.json")
    );
}

