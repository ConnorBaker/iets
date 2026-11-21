mod hermetic;

use hermetic::{Iets, bin, ok, stdout};
use rstest::rstest;
use std::fs;
use tempfile::tempdir;

#[rstest]
#[case("1")]
#[case("4")]
fn records_both_jobs_reading_a_shared_thunk(bin: Iets, #[case] workers: &str) {
    let dir = tempdir().unwrap();
    let file = |name| dir.path().join(name);
    fs::write(file("payload"), "old").unwrap();
    fs::write(
        file("expr.nix"),
        "let shared = builtins.readFile ./payload; in [ shared shared ]",
    )
    .unwrap();
    let eval = || {
        let mut cmd = bin.cmd();
        cmd.args(["eval", "--json", "--no-cache", "--workers", workers])
            .arg(file("expr.nix"));
        cmd
    };
    fs::create_dir(file("jobs")).unwrap();
    let captured = ok(eval()
        .env("IETS_FINE_TRACE", "")
        .env("IETS_FINE_DAG", file("dag"))
        .env("IETS_FINE_JOBS", file("jobs")));
    assert_eq!(stdout(&captured), r#"["old","old"]"#);
    assert_eq!(fs::read_dir(file("jobs")).unwrap().count(), 2);
    fs::write(file("payload"), "new").unwrap();
    assert_eq!(stdout(&ok(&mut eval())), r#"["new","new"]"#);
    fs::write(
        file("delta"),
        format!("s\tV\t3\ns\tCM\t{}\n", file("payload").display()),
    )
    .unwrap();
    let checked = stdout(&ok(bin
        .cmd()
        .arg("fine-check")
        .arg(file("delta"))
        .arg(file("dag"))));
    let dirty: Vec<_> = checked
        .lines()
        .map(|line| line.split('\t').nth(1).unwrap())
        .collect();
    assert_eq!(dirty, ["0", "1"]);
}
