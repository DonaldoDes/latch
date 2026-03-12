//! Tests for M0 — Open Source Foundation deliverables.
//! Validates that all required files exist and contain expected content.

use std::fs;
use std::path::Path;

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

// --- US-000: LICENSE ---

#[test]
fn license_file_exists() {
    assert!(repo_root().join("LICENSE").exists());
}

#[test]
fn license_is_mit_with_correct_copyright() {
    let content = fs::read_to_string(repo_root().join("LICENSE")).unwrap();
    assert!(content.contains("MIT License"));
    assert!(content.contains("2026 latch contributors"));
}

#[test]
fn changelog_exists_and_uses_keep_a_changelog() {
    let content = fs::read_to_string(repo_root().join("CHANGELOG.md")).unwrap();
    assert!(content.contains("Keep a Changelog"));
    assert!(content.contains("Semantic Versioning"));
}

// --- US-001: Repo Structure ---

#[test]
fn readme_has_ci_and_license_badges() {
    let content = fs::read_to_string(repo_root().join("README.md")).unwrap();
    assert!(content.contains("actions/workflows/ci.yml"));
    assert!(content.contains("License"));
}

#[test]
fn readme_has_required_sections() {
    let content = fs::read_to_string(repo_root().join("README.md")).unwrap();
    assert!(content.contains("## Installation"));
    assert!(content.contains("## Quick Start"));
    assert!(content.contains("## Commands"));
    assert!(content.contains("## Philosophy"));
    assert!(content.contains("## Contributing"));
    assert!(content.contains("## License"));
}

#[test]
fn readme_lists_all_commands() {
    let content = fs::read_to_string(repo_root().join("README.md")).unwrap();
    for cmd in &[
        "new", "attach", "detach", "list", "kill", "history", "rename",
    ] {
        assert!(
            content.contains(&format!("latch {}", cmd)),
            "README missing command: latch {}",
            cmd
        );
    }
}

#[test]
fn contributing_has_prerequisites_and_conventions() {
    let content = fs::read_to_string(repo_root().join("CONTRIBUTING.md")).unwrap();
    assert!(content.contains("Rust"));
    assert!(content.contains("cargo fmt"));
    assert!(content.contains("cargo clippy"));
}

#[test]
fn contributing_has_semver_and_crates_io() {
    let content = fs::read_to_string(repo_root().join("CONTRIBUTING.md")).unwrap();
    assert!(content.contains("semver") || content.contains("Semver") || content.contains("MAJOR"));
    assert!(content.contains("crates.io"));
    assert!(content.contains("CARGO_REGISTRY_TOKEN"));
}

#[test]
fn contributing_has_rust_version() {
    let content = fs::read_to_string(repo_root().join("CONTRIBUTING.md")).unwrap();
    assert!(content.contains("1.85"));
}

#[test]
fn contributing_has_dependency_policy() {
    let content = fs::read_to_string(repo_root().join("CONTRIBUTING.md")).unwrap();
    assert!(
        content.contains("Dependabot") || content.contains("dependabot"),
        "CONTRIBUTING missing Dependency Policy section"
    );
    assert!(content.contains("cargo deny"));
}

#[test]
fn code_of_conduct_exists_and_is_contributor_covenant() {
    let content = fs::read_to_string(repo_root().join("CODE_OF_CONDUCT.md")).unwrap();
    assert!(content.contains("Contributor Covenant"));
    assert!(content.contains("2.1"));
    assert!(content.contains("@DonaldoDes") || content.contains("DonaldoDes"));
}

// --- US-002: Issue Templates ---

#[test]
fn bug_report_template_is_github_issue_form() {
    let content =
        fs::read_to_string(repo_root().join(".github/ISSUE_TEMPLATE/bug_report.yml")).unwrap();
    assert!(content.contains("type: input") || content.contains("type: textarea"));
    assert!(content.contains("Steps to reproduce") || content.contains("steps"));
}

#[test]
fn feature_request_template_is_github_issue_form() {
    let content =
        fs::read_to_string(repo_root().join(".github/ISSUE_TEMPLATE/feature_request.yml")).unwrap();
    assert!(content.contains("type: textarea"));
}

#[test]
fn pr_template_exists() {
    assert!(repo_root()
        .join(".github/PULL_REQUEST_TEMPLATE.md")
        .exists());
}

// --- US-003: CI ---

#[test]
fn ci_has_matrix_os_and_rust() {
    let content = fs::read_to_string(repo_root().join(".github/workflows/ci.yml")).unwrap();
    assert!(content.contains("macos-latest"));
    assert!(content.contains("ubuntu-latest"));
    assert!(content.contains("stable"));
    assert!(content.contains("beta"));
    assert!(content.contains("fail-fast: false"));
}

#[test]
fn ci_uses_rust_cache() {
    let content = fs::read_to_string(repo_root().join(".github/workflows/ci.yml")).unwrap();
    assert!(content.contains("Swatinem/rust-cache"));
}

#[test]
fn ci_has_cargo_audit_step() {
    let content = fs::read_to_string(repo_root().join(".github/workflows/ci.yml")).unwrap();
    assert!(content.contains("cargo audit"));
}

#[test]
fn ci_has_cargo_deny_step() {
    let content = fs::read_to_string(repo_root().join(".github/workflows/ci.yml")).unwrap();
    assert!(content.contains("cargo deny"));
}

#[test]
fn ci_lint_job_exists() {
    let content = fs::read_to_string(repo_root().join(".github/workflows/ci.yml")).unwrap();
    // Lint should be a separate job
    assert!(content.contains("lint:") || content.contains("lint"));
    assert!(content.contains("cargo fmt --check"));
    assert!(content.contains("cargo clippy"));
}

// --- US-004: Release Workflow ---

#[test]
fn cargo_toml_has_rust_version() {
    let content = fs::read_to_string(repo_root().join("Cargo.toml")).unwrap();
    assert!(content.contains("rust-version = \"1.85\""));
}

#[test]
fn release_workflow_exists() {
    let content = fs::read_to_string(repo_root().join(".github/workflows/release.yml")).unwrap();
    assert!(content.contains("tags:"));
    assert!(content.contains("v*"));
}

#[test]
fn release_workflow_has_matrix_targets() {
    let content = fs::read_to_string(repo_root().join(".github/workflows/release.yml")).unwrap();
    assert!(content.contains("aarch64-apple-darwin"));
    assert!(content.contains("x86_64-apple-darwin"));
    assert!(content.contains("x86_64-unknown-linux-gnu"));
}

#[test]
fn release_workflow_has_gh_release_and_publish() {
    let content = fs::read_to_string(repo_root().join(".github/workflows/release.yml")).unwrap();
    assert!(content.contains("softprops/action-gh-release"));
    assert!(content.contains("cargo publish"));
    assert!(content.contains("CARGO_REGISTRY_TOKEN"));
}

#[test]
fn roadmap_exists() {
    let content = fs::read_to_string(repo_root().join("ROADMAP.md")).unwrap();
    assert!(content.contains("milestone") || content.contains("Milestone"));
}

// --- US-005: Security ---

#[test]
fn security_md_exists_with_required_sections() {
    let content = fs::read_to_string(repo_root().join("SECURITY.md")).unwrap();
    assert!(content.contains("Supported Versions"));
    assert!(content.contains("Reporting a Vulnerability") || content.contains("Reporting"));
    assert!(content.contains("90"));
    assert!(content.contains("Private") || content.contains("Advisory"));
}

#[test]
fn deny_toml_exists_with_license_config() {
    let content = fs::read_to_string(repo_root().join("deny.toml")).unwrap();
    assert!(content.contains("[licenses]"));
    assert!(content.contains("MIT"));
    assert!(content.contains("Apache-2.0"));
    assert!(content.contains("[advisories]"));
    assert!(content.contains("[bans]"));
    assert!(content.contains("[sources]"));
}

// --- US-006: Dependabot ---

#[test]
fn dependabot_config_exists() {
    let content = fs::read_to_string(repo_root().join(".github/dependabot.yml")).unwrap();
    assert!(content.contains("cargo"));
    assert!(content.contains("github-actions"));
    assert!(content.contains("weekly"));
    assert!(content.contains("dependencies"));
}
