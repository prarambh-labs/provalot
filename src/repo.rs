use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Nearest ancestor of `cwd` containing `.git`, else `cwd` itself.
pub fn find_root(cwd: &Path) -> PathBuf {
    let mut cur = Some(cwd);
    while let Some(dir) = cur {
        if dir.join(".git").exists() {
            return dir.to_path_buf();
        }
        cur = dir.parent();
    }
    cwd.to_path_buf()
}

/// Path relative to `root` when possible, forward slashes.
pub fn rel(root: &Path, path: &Path) -> String {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let p = match abs.strip_prefix(root) {
        Ok(x) => x.to_path_buf(),
        // The root and the path can name the same directory through different symlinks
        // (macOS `/tmp` -> `/private/tmp`, a symlinked home). Retry canonicalized.
        Err(_) => canonical_rel(root, &abs).unwrap_or(abs),
    };
    p.to_string_lossy().replace('\\', "/")
}

fn canonical_rel(root: &Path, abs: &Path) -> Option<PathBuf> {
    let root = root.canonicalize().ok()?;
    // The file may not exist yet (an added file), so fall back to canonicalizing its parent.
    let path = abs
        .canonicalize()
        .ok()
        .or_else(|| Some(abs.parent()?.canonicalize().ok()?.join(abs.file_name()?)))?;
    path.strip_prefix(&root).ok().map(|x| x.to_path_buf())
}

pub fn sha256_file(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(format!("{:x}", Sha256::digest(&bytes)))
}

pub fn sha256_str(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_is_nearest_git_ancestor_or_cwd() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(repo.join("a/b")).unwrap();
        assert_eq!(find_root(&repo.join("a/b")), repo);
        let plain = dir.path().join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        assert_eq!(find_root(&plain), plain);
    }

    #[test]
    fn rel_and_hash() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("src/app.py");
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, "print(1)\n").unwrap();
        assert_eq!(rel(dir.path(), &f), "src/app.py");
        assert_eq!(rel(dir.path(), std::path::Path::new("src/app.py")), "src/app.py");
        assert_eq!(sha256_file(&f), Some(sha256_str("print(1)\n")));
        assert_eq!(sha256_file(&dir.path().join("nope")), None);
    }

    /// macOS `/tmp` and `/var`, and symlinked homes, make the root and the tool's
    /// path disagree textually while naming the same file.
    #[cfg(unix)]
    #[test]
    fn rel_resolves_a_symlinked_root() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(real.join("src")).unwrap();
        std::fs::write(real.join("src/app.py"), "x").unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        assert_eq!(rel(&link, &real.join("src/app.py")), "src/app.py");
        assert_eq!(rel(&real, &link.join("src/app.py")), "src/app.py");
        // A file that does not exist yet (apply_patch "Add File") still resolves.
        assert_eq!(rel(&link, &real.join("src/new.py")), "src/new.py");
        // A path genuinely outside the root stays absolute.
        let outside = dir.path().join("elsewhere.py");
        std::fs::write(&outside, "x").unwrap();
        assert_eq!(rel(&real, &outside), outside.to_string_lossy());
    }
}
