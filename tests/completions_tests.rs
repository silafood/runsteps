mod common;

#[test]
fn completions_bash_contains_runsteps() {
    let dir = common::tmpdir("comp-bash");
    let out = common::run(&["completions", "bash"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let combined = common::stdout(&out) + &common::stderr(&out);
    assert!(
        combined.contains("runsteps"),
        "bash completions missing 'runsteps', got: {}",
        &combined[..combined.len().min(200)]
    );
}

#[test]
fn completions_zsh_contains_compdef() {
    let dir = common::tmpdir("comp-zsh");
    let out = common::run(&["completions", "zsh"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let combined = common::stdout(&out) + &common::stderr(&out);
    assert!(
        combined.contains("#compdef runsteps") || combined.contains("_runsteps"),
        "zsh completions missing compdef/function, got: {}",
        &combined[..combined.len().min(200)]
    );
}

#[test]
fn completions_fish_contains_complete_c() {
    let dir = common::tmpdir("comp-fish");
    let out = common::run(&["completions", "fish"], &dir);
    assert!(out.status.success(), "stderr: {}", common::stderr(&out));
    let combined = common::stdout(&out) + &common::stderr(&out);
    assert!(
        combined.contains("complete") && combined.contains("runsteps"),
        "fish completions missing 'complete -c runsteps', got: {}",
        &combined[..combined.len().min(200)]
    );
}

#[test]
fn completions_invalid_shell_exits_nonzero() {
    let dir = common::tmpdir("comp-invalid");
    let out = common::run(&["completions", "invalid_shell"], &dir);
    assert!(
        !out.status.success(),
        "expected nonzero exit for invalid shell, got success"
    );
}
