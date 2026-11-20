//! A changed alias, rendered child, and list selector affect different jobs.
#![cfg(unix)]

mod hermetic;
use hermetic::{bin, ok, stdout};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn chooser_alias_membership_and_nested_render_demands() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let bin = bin();
    let graph = dir.join("graph");
    let root = dir.join("root.nix");
    let aliases = dir.join("aliases.nix");
    let guard = dir.join("switch.nix");
    let nested = dir.join("meta.nix");
    fs::write(
        &root,
        r#"let aliases = import ./aliases.nix; in
        if import ./switch.nix then
          [ (aliases ? "alias with space") { label = import ./meta.nix; } 17 ]
        else [ true { label = 3; } 18 ]"#,
    )
    .unwrap();
    let initial_aliases = "{ a0=0;a1=1;a2=2;a3=3;a4=4;a5=5;a6=6;a7=7; }";
    fs::write(&aliases, initial_aliases).unwrap();
    fs::write(&guard, "true").unwrap();
    fs::write(&nested, "1").unwrap();
    let evaluate = |trace: bool| {
        let mut cmd = bin.cmd();
        cmd.args(["eval", "--json", "--no-cache", "--workers", "1"])
            .arg(&root);
        if trace {
            cmd.env("IETS_FINE_TRACE", "").env("IETS_FINE_DAG", &graph);
        }
        stdout(&ok(&mut cmd))
    };
    let expected = r#"[false,{"label":1},17]"#;
    assert_eq!(evaluate(true), expected);
    let cases: &[(&PathBuf, &str, &str, &str, &[usize])] = &[
        (
            &aliases,
            initial_aliases,
            r#"{ a0=0;a1=1;a2=2;a3=3;a4=4;a5=5;a6=6;a7=7;"alias with space"=throw "not forced"; }"#,
            r#"[true,{"label":1},17]"#,
            &[0],
        ),
        (&nested, "1", "2", r#"[false,{"label":2},17]"#, &[1]),
        (
            &guard,
            "true",
            "false",
            r#"[true,{"label":3},18]"#,
            &[0, 1, 2],
        ),
    ];
    let diff = Path::new(env!("CARGO_MANIFEST_DIR")).join("bench/incremental/fcmp.awk");
    for &(file, before, after, fresh, changed_jobs) in cases {
        let regions =
            || String::from_utf8(ok(bin.cmd().arg("fine-regions").arg(file)).stdout).unwrap();
        fs::write(dir.join("old"), regions()).unwrap();
        fs::write(file, after).unwrap();
        assert_eq!(evaluate(false), fresh);
        fs::write(dir.join("new"), regions()).unwrap();
        let delta = String::from_utf8(
            ok(Command::new("awk")
                .current_dir(dir)
                .arg("-v")
                .arg(format!("f={}", file.display()))
                .arg("-f")
                .arg(&diff)
                .args(["old", "new"]))
            .stdout,
        )
        .unwrap();
        let mut changes = format!(
            "r\tV\t2\nr\tCM\t{}\nr\tCT\t{}\nr\tCX\t{}\t0\n",
            file.display(),
            file.display(),
            file.display()
        );
        for row in delta.lines() {
            changes += &format!("r\t{row}\n");
        }
        fs::write(dir.join("changes"), changes).unwrap();
        let report = stdout(&ok(bin.cmd().current_dir(dir).args([
            "fine-check",
            "changes",
            "graph",
        ])));
        let flagged: BTreeSet<usize> = report
            .lines()
            .map(|l| l.split('\t').nth(1).unwrap().parse().unwrap())
            .collect();
        assert_eq!(
            flagged,
            changed_jobs.iter().copied().collect(),
            "{file:?}: {report:?}"
        );
        fs::write(file, before).unwrap();
    }
}

#[test]
fn volatile_selection_and_nested_json_reach_their_consumers() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("root.nix");
    let graph = tmp.path().join("graph");
    let changes = tmp.path().join("changes");
    let bin = bin();
    fs::write(&changes, "r\tV\t2\n").unwrap();
    for (expr, expected) in [
        (
            "if builtins.currentTime > 0 then [ 1 2 ] else [ 3 4 ]",
            &[0, 1][..],
        ),
        ("[ { time=builtins.currentTime; } 7 ]", &[0][..]),
    ] {
        fs::write(&root, expr).unwrap();
        ok(bin
            .cmd()
            .args([
                "eval",
                "--impure",
                "--strict",
                "--json",
                "--no-cache",
                "--workers",
                "1",
            ])
            .arg(&root)
            .env("IETS_FINE_TRACE", "")
            .env("IETS_FINE_DAG", &graph));
        let report = stdout(&ok(bin.cmd().arg("fine-check").arg(&changes).arg(&graph)));
        let flagged: BTreeSet<usize> = report
            .lines()
            .map(|line| line.split('\t').nth(1).unwrap().parse().unwrap())
            .collect();
        assert_eq!(flagged, expected.iter().copied().collect(), "{expr}");
    }
}
