use std::process::{Command, Output};

fn run(arguments: &[&str]) -> Output {
    Command::new(std::fs::canonicalize(env!("CARGO_BIN_EXE_trail")).unwrap())
        .args(arguments)
        .output()
        .unwrap()
}

#[test]
fn help_aliases_streams_placeholders_and_width_agree() {
    for command in [
        None,
        Some("build"),
        Some("serve"),
        Some("lint"),
        Some("help"),
    ] {
        let prefix: Vec<_> = command.into_iter().collect();
        let mut long = prefix.clone();
        long.push("--help");
        let mut short = prefix;
        short.push("-h");
        let mut alias = vec!["help"];
        alias.extend(command);
        let expected = run(&long);
        assert!(expected.status.success());
        assert!(expected.stderr.is_empty());
        assert_eq!(run(&short).stdout, expected.stdout);
        assert_eq!(run(&alias).stdout, expected.stdout);
        let text = String::from_utf8(expected.stdout).unwrap();
        assert!(text.contains("See https://learn.peios.org/trail/"));
        assert!(
            text.lines().all(|line| line.chars().count() <= 80),
            "{text}"
        );
        for forbidden in ["[OPTIONS]", "[ROOT]", "<DIR>", "[default:"] {
            assert!(!text.contains(forbidden), "{text}");
        }
    }
    let missing = run(&[]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(missing.stdout.is_empty());
    assert_eq!(missing.stderr, run(&["--help"]).stdout);
}

#[test]
fn version_aliases_and_usage_errors_have_consistent_statuses() {
    let version = run(&["--version"]);
    assert!(version.status.success());
    assert!(version.stderr.is_empty());
    assert_eq!(
        version.stdout,
        format!("trail {}\n", env!("CARGO_PKG_VERSION")).as_bytes()
    );
    assert_eq!(run(&["-V"]).stdout, version.stdout);
    for command in ["build", "serve", "lint", "help"] {
        assert_eq!(run(&[command, "--version"]).stdout, version.stdout);
        assert_eq!(run(&[command, "-V"]).stdout, version.stdout);
    }
    for arguments in [
        vec!["unknown"],
        vec!["build", "--unknown"],
        vec!["build", "--out"],
        vec!["serve", "--addr", "not-an-address"],
        vec!["build", "--strict", "--strict"],
        vec!["help", "unknown"],
    ] {
        let output = run(&arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty());
        let text = String::from_utf8(output.stderr).unwrap();
        assert!(text.starts_with("trail: "), "{text}");
        assert_eq!(text.lines().count(), 2, "{text}");
        assert!(
            text.ends_with("Try 'trail --help' for more information.\n"),
            "{text}"
        );
        assert!(!text.contains("error:"), "{text}");
    }
}

#[test]
fn operational_errors_use_display_text_and_stderr() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing");
    let output = run(&["build", missing.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(text.starts_with("trail: "), "{text}");
    assert!(text.contains("No such file or directory"), "{text}");
    assert!(!text.contains("os error"), "{text}");
    assert!(!text.contains("Error:"), "{text}");
    assert_eq!(text.lines().count(), 1);
}

#[cfg(unix)]
#[test]
fn diagnostics_use_the_invoked_basename() {
    let directory = tempfile::tempdir().unwrap();
    let alias = directory.path().join("docs-tool");
    let binary = std::fs::canonicalize(env!("CARGO_BIN_EXE_trail")).unwrap();
    std::os::unix::fs::symlink(binary, &alias).unwrap();
    let output = Command::new(alias).arg("--unknown").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(text.starts_with("docs-tool: "));
    assert!(text.contains("Try 'docs-tool --help'"));
}

#[test]
fn lint_findings_and_totals_are_diagnostics() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("trail.toml"),
        "[lint]\nrules = [\"exclamation\"]\n",
    )
    .unwrap();
    std::fs::write(
        directory.path().join("article.md"),
        "---\ntitle: Test\n---\nBad!\n",
    )
    .unwrap();
    let output = run(&["lint", directory.path().to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let text = String::from_utf8(output.stderr).unwrap();
    assert!(text.contains("trail: article.md:4:"), "{text}");
    assert!(text.ends_with("trail: 1 error, 0 warnings\n"), "{text}");
}

#[test]
fn manual_documents_every_help_option_and_the_package_version() {
    let manual = include_str!("../man/trail.1").replace("\\-", "-");
    assert!(manual.contains(&format!("Peios trail {}", env!("CARGO_PKG_VERSION"))));
    for command in ["build", "serve", "lint", "help"] {
        let output = run(&[command, "--help"]);
        let help = String::from_utf8(output.stdout).unwrap();
        for option in help
            .split_whitespace()
            .filter(|word| word.starts_with("--"))
        {
            assert!(manual.contains(option), "manual omits {option}");
        }
    }
}
