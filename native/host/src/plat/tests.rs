//! The platform layer end to end: what a child process gets on this system.
/// A script handed to the shell reaches it byte for byte (quotes, backslashes, a drive path), and so do its arguments.
#[test] fn a_script_and_its_arguments_reach_the_shell_unchanged() {
    let script = r#"case "$1" in [a-zA-Z]:[\\/]*) printf 'drive|%s|%s' "$1" "$2";; *) printf 'other|%s' "$1";; esac"#;
    let o = crate::proc::std_cmd(crate::proc::Kind::Local, "sh").args(["-c", script, "sh", r"C:\Users\RUNNER~1\x y", r#"[\\/] "q" \"#]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout), r#"drive|C:\Users\RUNNER~1\x y|[\\/] "q" \"#, "{}", String::from_utf8_lossy(&o.stderr));
}
/// A `#!` script runs as a program (Windows: through the interpreter its first line names).
#[test] fn a_shebang_script_runs_as_a_program() {
    let d = std::env::temp_dir().join(format!("bp-plat-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap();
    let f = d.join("hello"); std::fs::write(&f, "#!/bin/sh\necho \"hi $1\"\n").unwrap();
    { use crate::plat::fs::PermissionsExt; std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap(); }
    let o = crate::proc::std_cmd(crate::proc::Kind::Local, &f).arg("there").output().unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "hi there");
    let _ = std::fs::remove_dir_all(&d);
}
