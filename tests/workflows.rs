const CI_WORKFLOW: &str = include_str!("../.github/workflows/ci.yml");
const RELEASE_WORKFLOW: &str = include_str!("../.github/workflows/release.yml");

#[test]
fn ci_checks_all_supported_operating_systems_with_the_lockfile() {
    for runner in ["ubuntu-24.04", "windows-2025", "macos-15-intel"] {
        assert!(CI_WORKFLOW.contains(runner), "missing CI runner {runner}");
    }

    for command in [
        "cargo check --locked --all-targets",
        "cargo test --locked --all-targets",
        "cargo package --locked",
    ] {
        assert!(
            CI_WORKFLOW.contains(command),
            "missing CI command: {command}"
        );
    }
}

#[test]
fn release_builds_binaries_for_linux_windows_and_macos() {
    for target in [
        "x86_64-unknown-linux-gnu",
        "x86_64-pc-windows-msvc",
        "x86_64-apple-darwin",
    ] {
        assert!(
            RELEASE_WORKFLOW.contains(target),
            "missing release target {target}"
        );
    }

    assert!(RELEASE_WORKFLOW.contains("cargo test --locked --all-targets"));
    assert!(RELEASE_WORKFLOW.contains("cargo build --locked --release --bin potoo-pseint"));
}
