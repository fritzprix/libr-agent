use std::fs;

use tauri_mcp_agent_lib::agent::references::list_relative_paths_in_root;
use tempfile::tempdir;

#[tokio::test]
async fn file_indexing_respects_root_gitignore_and_skips_heavy_directories() {
    let temp_dir = tempdir().expect("temp dir");
    let root = temp_dir.path();

    fs::create_dir_all(root.join("src")).expect("src dir");
    fs::create_dir_all(root.join("generated")).expect("generated dir");
    fs::create_dir_all(root.join("node_modules/pkg")).expect("node_modules dir");
    fs::create_dir_all(root.join("dist/assets")).expect("dist dir");

    fs::write(root.join(".gitignore"), "generated/\n").expect("write gitignore");
    fs::write(root.join("src/main.ts"), "export const tracked = true;\n")
        .expect("write tracked file");
    fs::write(
        root.join("generated/ignored.ts"),
        "export const ignored = true;\n",
    )
    .expect("write ignored file");
    fs::write(
        root.join("node_modules/pkg/index.js"),
        "module.exports = {};\n",
    )
    .expect("write node_modules file");
    fs::write(root.join("dist/assets/app.js"), "console.log('bundle');\n")
        .expect("write dist file");

    let paths = list_relative_paths_in_root(root, 8)
        .await
        .expect("list workspace files");

    assert!(
        paths.contains(&"src/main.ts".to_string()),
        "tracked file should remain indexable: {paths:?}"
    );
    assert!(
        !paths.contains(&"generated/ignored.ts".to_string()),
        "root gitignored file should be excluded: {paths:?}"
    );
    assert!(
        !paths.iter().any(|path| path.starts_with("node_modules/")),
        "node_modules should be excluded from indexing: {paths:?}"
    );
    assert!(
        !paths.iter().any(|path| path.starts_with("dist/")),
        "dist should be excluded from indexing: {paths:?}"
    );
}

#[tokio::test]
async fn file_indexing_respects_nested_gitignore_without_hiding_siblings() {
    let temp_dir = tempdir().expect("temp dir");
    let root = temp_dir.path();

    fs::create_dir_all(root.join("generated/ignored")).expect("ignored dir");
    fs::write(root.join("generated/.gitignore"), "ignored/\n").expect("write nested gitignore");
    fs::write(
        root.join("generated/visible.ts"),
        "export const visible = true;\n",
    )
    .expect("write visible file");
    fs::write(
        root.join("generated/ignored/hidden.ts"),
        "export const hidden = true;\n",
    )
    .expect("write hidden file");

    let paths = list_relative_paths_in_root(root, 8)
        .await
        .expect("list workspace files");

    assert!(
        paths.contains(&"generated/visible.ts".to_string()),
        "visible sibling file should remain indexable: {paths:?}"
    );
    assert!(
        !paths.contains(&"generated/ignored/hidden.ts".to_string()),
        "nested gitignored file should be excluded: {paths:?}"
    );
}
