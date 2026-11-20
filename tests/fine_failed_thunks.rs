//! Every consumer of a caught failure retains its input dependencies.

mod hermetic;

use hermetic::{Iets, bin, ok, stdout};
use rstest::rstest;
use std::fs;
use tempfile::tempdir;

#[rstest]
fn caught_shared_failure_invalidates_every_reader(bin: Iets) {
    let dir = tempdir().unwrap();
    let path = |name| dir.path().join(name);
    fs::write(path("flag"), "bad").unwrap();
    fs::write(
        path("expr.nix"),
        r#"let
          shared = if builtins.readFile ./flag == "bad" then throw "bad input" else 7;
        in (map (_: (builtins.tryEval shared).success) [ 0 1 ]) ++ [ 7 ]"#,
    )
    .unwrap();
    let eval = || {
        let mut cmd = bin.cmd();
        cmd.args(["eval", "--json", "--no-cache", "--workers", "1"])
            .arg(path("expr.nix"));
        cmd
    };
    assert_eq!(
        stdout(&ok(eval()
            .env("IETS_FINE_TRACE", "")
            .env("IETS_FINE_DAG", path("dag")))),
        "[false,false,7]"
    );
    fs::write(path("flag"), "good").unwrap();
    assert_eq!(stdout(&ok(&mut eval())), "[true,true,7]");
    fs::write(
        path("delta"),
        format!("s\tV\t2\ns\tCM\t{}\n", path("flag").display()),
    )
    .unwrap();
    let checked = stdout(&ok(bin
        .cmd()
        .current_dir(dir.path())
        .args(["fine-check", "delta", "dag"])));
    assert_eq!(checked, "dag\t0\ts\ndag\t1\ts");
}
