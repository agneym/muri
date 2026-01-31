use rustc_hash::FxHashSet;
use std::path::Path;

/// Detect dependencies from package.json in the given directory
pub fn detect_dependencies(cwd: &Path) -> FxHashSet<String> {
    let pkg_path = cwd.join("package.json");
    if !pkg_path.exists() {
        return FxHashSet::default();
    }

    let content = match std::fs::read_to_string(&pkg_path) {
        Ok(c) => c,
        Err(_) => return FxHashSet::default(),
    };

    let pkg: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return FxHashSet::default(),
    };

    let mut deps = FxHashSet::default();

    for field in ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"] {
        if let Some(obj) = pkg.get(field).and_then(|v| v.as_object()) {
            for key in obj.keys() {
                deps.insert(key.clone());
            }
        }
    }

    deps
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_detect_dependencies_with_all_fields() {
        let dir = tempdir().unwrap();
        let pkg_json = r#"{
            "dependencies": {
                "lodash": "^4.17.21"
            },
            "devDependencies": {
                "sass": "^1.60.0"
            },
            "peerDependencies": {
                "vue": "^3.0.0"
            },
            "optionalDependencies": {
                "@storybook/react": "^8.0.0"
            }
        }"#;
        fs::write(dir.path().join("package.json"), pkg_json).unwrap();

        let deps = detect_dependencies(dir.path());

        assert!(deps.contains("lodash"));
        assert!(deps.contains("sass"));
        assert!(deps.contains("vue"));
        assert!(deps.contains("@storybook/react"));
    }

    #[test]
    fn test_detect_dependencies_no_package_json() {
        let dir = tempdir().unwrap();
        let deps = detect_dependencies(dir.path());
        assert!(deps.is_empty());
    }

    #[test]
    fn test_detect_dependencies_invalid_json() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "invalid json").unwrap();

        let deps = detect_dependencies(dir.path());
        assert!(deps.is_empty());
    }
}
