use muri::{MuriConfig, find_unused_files};
use std::path::PathBuf;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/scss")
}

#[test]
fn test_scss_integration_finds_unused_files() {
    let cwd = fixture_path();

    let config = MuriConfig {
        entry: vec!["index.ts".to_string()],
        project: vec!["**/*.{ts,tsx,scss}".to_string()],
        cwd: cwd.clone(),
        ignore: Vec::new(),
        compilers: Default::default(),
    };

    let report = find_unused_files(config).expect("Should find unused files");

    // Convert to relative paths for easier comparison
    let cwd_canonical = cwd.canonicalize().unwrap();
    let unused_relative: Vec<String> = report
        .unused_files
        .iter()
        .map(|p| p.strip_prefix(&cwd_canonical).unwrap_or(p).to_string_lossy().to_string())
        .collect();

    // Only unused.ts and styles/unused.scss should be detected as unused
    // index.ts -> styles/index.scss -> _variables.scss, _mixins.scss
    // index.ts -> helper.ts
    assert!(
        unused_relative.iter().any(|p| p.ends_with("unused.ts")),
        "unused.ts should be detected as unused, got: {unused_relative:?}",
    );
    assert!(
        unused_relative.iter().any(|p| p.ends_with("unused.scss")),
        "styles/unused.scss should be detected as unused, got: {unused_relative:?}",
    );

    // These files should NOT be detected as unused
    assert!(
        !unused_relative.iter().any(|p| p.ends_with("index.ts")),
        "index.ts should not be detected as unused"
    );
    assert!(
        !unused_relative.iter().any(|p| p.ends_with("helper.ts")),
        "helper.ts should not be detected as unused"
    );
    assert!(
        !unused_relative.iter().any(|p| p.ends_with("index.scss")),
        "styles/index.scss should not be detected as unused"
    );

    // Verify count
    assert_eq!(report.unused_count, 2, "Expected exactly 2 unused files");
}

#[test]
fn test_scss_integration_finds_reachable_files() {
    let cwd = fixture_path();

    let config = MuriConfig {
        entry: vec!["index.ts".to_string()],
        project: vec!["**/*.{ts,tsx,scss}".to_string()],
        cwd: cwd.clone(),
        ignore: Vec::new(),
        compilers: Default::default(),
    };

    let reachable = muri::find_reachable_files(config).expect("Should find reachable files");

    // Convert to relative paths for easier comparison
    let cwd_canonical = cwd.canonicalize().unwrap();
    let reachable_relative: Vec<String> = reachable
        .iter()
        .map(|p| p.strip_prefix(&cwd_canonical).unwrap_or(p).to_string_lossy().to_string())
        .collect();

    // These files should be reachable from index.ts
    assert!(
        reachable_relative.iter().any(|p| p.ends_with("index.ts")),
        "index.ts should be reachable"
    );
    assert!(
        reachable_relative.iter().any(|p| p.ends_with("helper.ts")),
        "helper.ts should be reachable"
    );
    assert!(
        reachable_relative.iter().any(|p| p.ends_with("index.scss")),
        "styles/index.scss should be reachable, got: {reachable_relative:?}",
    );
    // Note: SCSS partials (_variables.scss, _mixins.scss) may or may not be resolved
    // depending on the resolver configuration - we test the main chain works
}
