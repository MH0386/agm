use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

fn agm() -> Command {
    Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap()
}

fn assert_invalid_source(source: &str, expected_error: &str) {
    let mut cmd = agm();
    cmd.args([
        "skill",
        "add",
        source,
        "--skill",
        "../must-not-reach-network",
    ]);
    cmd.assert().failure().stderr(contains(expected_error));
}

#[test]
fn cli_no_args_shows_help() {
    let mut cmd = agm();
    cmd.assert()
        .failure()
        .stderr(contains("No command provided"));
}

#[test]
fn cli_help_flag() {
    let mut cmd = agm();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(contains("Usage:"))
        .stdout(contains("Commands:"))
        .stdout(contains("Options:"));
}

#[test]
fn cli_version_flag() {
    let mut cmd = agm();
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn cli_debug_flag_is_accepted() {
    let mut cmd = agm();
    cmd.args(["--debug", "--help"]);
    cmd.assert().success();
}

#[test]
fn init_subcommand_help() {
    let mut cmd = agm();
    cmd.args(["init", "--help"]);
    cmd.assert()
        .success()
        .stdout(contains("Initialize the AGM configuration"));
}

#[test]
fn skill_subcommand_help() {
    let mut cmd = agm();
    cmd.args(["skill", "--help"]);
    cmd.assert()
        .success()
        .stdout(contains("Skill commands"))
        .stdout(contains("list"))
        .stdout(contains("add"));
}

#[test]
fn skill_list_help() {
    let mut cmd = agm();
    cmd.args(["skill", "list", "--help"]);
    cmd.assert()
        .success()
        .stdout(contains("List all managed skills"));
}

#[test]
fn skill_add_help() {
    let mut cmd = agm();
    cmd.args(["skill", "add", "--help"]);
    cmd.assert()
        .success()
        .stdout(contains("Add a skill from a registry source"))
        .stdout(contains("<SOURCE>"))
        .stdout(contains("--skill"));
}

#[test]
fn skill_add_requires_source() {
    let mut cmd = agm();
    cmd.args(["skill", "add", "--skill", "my-skill"]);
    cmd.assert()
        .failure()
        .stderr(contains("required").or(contains("Usage:")));
}

#[test]
fn skill_add_requires_skill_name() {
    let mut cmd = agm();
    cmd.args(["skill", "add", "github:foo/bar"]);
    cmd.assert()
        .failure()
        .stderr(contains("required").or(contains("Usage:")));
}

#[test]
fn skill_add_rejects_malformed_standard_github_owners() {
    for source in [
        "github:-octo/repo",
        "github:octo-/repo",
        "github:octo--cat/repo",
    ] {
        assert_invalid_source(source, "Invalid GitHub owner");
    }
}

#[test]
fn skill_add_rejects_malformed_managed_github_owners() {
    for source in [
        "github:octo_ab/repo",
        "github:octo_abcdefghi/repo",
        "github:octo__admin/repo",
    ] {
        assert_invalid_source(source, "Invalid GitHub owner");
    }
}

#[test]
fn skill_add_rejects_invalid_github_repository_characters() {
    assert_invalid_source("github:octo/repo@name", "Invalid GitHub repository name");
}

#[test]
fn skill_add_rejects_excessive_github_identifier_lengths() {
    let owner = "o".repeat(40);
    let repo = "r".repeat(101);

    assert_invalid_source(&format!("github:{owner}/repo"), "Invalid GitHub owner");
    assert_invalid_source(
        &format!("github:octo/{repo}"),
        "Invalid GitHub repository name",
    );
}

#[test]
fn skill_add_rejects_path_traversal_skill_before_network() {
    let mut cmd = agm();
    cmd.args(["skill", "add", "github:octo/repo", "--skill", "../escape"]);
    cmd.assert()
        .failure()
        .stderr(contains("Invalid skill name `../escape`"));
}

#[test]
fn mcp_subcommand_help() {
    let mut cmd = agm();
    cmd.args(["mcp", "--help"]);
    cmd.assert()
        .success()
        .stdout(contains("MCP commands"))
        .stdout(contains("list"));
}

#[test]
fn mcp_list_help() {
    let mut cmd = agm();
    cmd.args(["mcp", "list", "--help"]);
    cmd.assert()
        .success()
        .stdout(contains("List all managed MCPs"));
}
