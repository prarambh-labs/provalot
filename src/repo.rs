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
    let p = abs.strip_prefix(root).map(|x| x.to_path_buf()).unwrap_or(abs);
    p.to_string_lossy().replace('\\', "/")
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
}
