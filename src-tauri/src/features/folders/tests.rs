use std::path::Path;

use super::*;
use crate::core::ErrorCode;
use crate::testing::TestRepo;

fn list_repo(repo: &TestRepo) -> FolderListing {
    list(Some(&repo.path().to_string_lossy())).unwrap()
}

fn names(listing: &FolderListing) -> Vec<&str> {
    listing.folders.iter().map(|f| f.name.as_str()).collect()
}

#[test]
fn lists_only_visible_folders_sorted_case_insensitively() {
    let repo = TestRepo::new();
    repo.write("beta/x.txt", "");
    repo.write("Alpha/x.txt", "");
    repo.write("gamma/x.txt", "");
    repo.write(".hidden/x.txt", "");
    repo.write("file.txt", "");

    assert_eq!(names(&list_repo(&repo)), ["Alpha", "beta", "gamma"]);
}

#[test]
fn entries_carry_full_paths() {
    let repo = TestRepo::new();
    repo.write("child/x.txt", "");
    let listing = list_repo(&repo);
    assert_eq!(
        Path::new(&listing.folders[0].path),
        repo.path().join("child")
    );
}

#[test]
fn flags_git_repos() {
    let repo = TestRepo::new();
    std::fs::create_dir_all(repo.path().join("with-git/.git")).unwrap();
    repo.write("worktree/.git", "gitdir: elsewhere");
    repo.write("plain/x.txt", "");

    let listing = list_repo(&repo);
    let flags: Vec<(&str, bool)> = listing
        .folders
        .iter()
        .map(|f| (f.name.as_str(), f.is_git_repo))
        .collect();
    assert_eq!(
        flags,
        [("plain", false), ("with-git", true), ("worktree", true)]
    );
    assert!(!listing.is_git_repo);

    let inner = list(Some(&repo.path().join("with-git").to_string_lossy())).unwrap();
    assert!(inner.is_git_repo);
}

#[test]
fn reports_parent_and_normalizes_trailing_separators() {
    let repo = TestRepo::new();
    let with_trailing = format!("{}{}", repo.path().display(), std::path::MAIN_SEPARATOR);
    let listing = list(Some(&with_trailing)).unwrap();

    assert_eq!(Path::new(&listing.path), repo.path());
    assert_eq!(
        listing.parent.as_deref().map(Path::new),
        repo.path().parent()
    );
}

#[test]
fn root_has_no_parent() {
    let root = locations().unwrap().roots.into_iter().next().unwrap();
    assert_eq!(list(Some(&root)).unwrap().parent, None);
}

#[test]
fn defaults_to_home() {
    let listing = list(None).unwrap();
    assert_eq!(listing.path, locations().unwrap().home);
}

#[test]
fn rejects_relative_paths_and_parent_dirs() {
    let repo = TestRepo::new();
    assert_eq!(
        list(Some("relative/dir")).unwrap_err().code,
        ErrorCode::InvalidPath
    );
    let sneaky = repo.path().join("a").join("..").join("b");
    assert_eq!(
        list(Some(&sneaky.to_string_lossy())).unwrap_err().code,
        ErrorCode::InvalidPath
    );
}

#[test]
fn missing_folder_is_an_io_error() {
    let repo = TestRepo::new();
    let missing = repo.path().join("missing");
    assert_eq!(
        list(Some(&missing.to_string_lossy())).unwrap_err().code,
        ErrorCode::Io
    );
}

#[test]
fn locations_include_an_existing_home_and_roots() {
    let locations = locations().unwrap();
    assert!(Path::new(&locations.home).is_dir());
    assert!(!locations.roots.is_empty());
    assert!(locations.roots.iter().all(|root| Path::new(root).is_dir()));
}
