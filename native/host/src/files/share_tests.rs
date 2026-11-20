use super::*;

fn tmp(n: &str) -> crate::plat::tmp::TempDir { crate::plat::tmp::canonical(&format!("share-{n}")) }

#[test] fn the_rule_is_applied_to_the_handle_opened() {
    let d = tmp("handle"); let (proj, out, home) = (d.join("proj"), d.join("out"), d.join("home"));
    for x in [&proj, &out, &home] { std::fs::create_dir_all(x).unwrap() }
    std::fs::write(proj.join("a.zip"), b"in").unwrap(); std::fs::write(out.join("b.zip"), b"out").unwrap();
    let roots = vec![proj.clone()];
    let o = open_checked(&roots, &home, &s(&proj.join("a.zip"))).unwrap();
    assert_eq!((o.real.clone(), o.md.len()), (proj.join("a.zip"), 2), "the handle's own path and size");
    // a path that is a link when opened is never followed: a swap of the checked file for a link to one outside fails at the open
    crate::plat::fs::symlink(out.join("b.zip"), proj.join("swapped.zip")).unwrap();
    assert!(open_real(&roots, &home, &proj.join("swapped.zip")).is_err(), "O_NOFOLLOW");
    // a file the handle says is outside the roots is refused even when opened by a path inside
    assert_eq!(open_real(&roots, &home, &out.join("b.zip")).err(), Some("that file is not in a project"));
    assert!(open_checked(&roots, &home, &s(&proj)).is_err(), "a folder");
}

#[test] fn the_store_is_private_from_its_first_byte_and_revokes_say_when_they_fail() {
    let d = tmp("mutate");
    let sh = Share { id: "a".into(), token: "t".repeat(48), path: "/p/a.zip".into(), name: "a.zip".into(), size: 1, thread: "t1".into(), created: 1, expires: u64::MAX, tailnet: false, url: String::new() };
    mutate(&d, |v| v.push(sh.clone())).unwrap();
    assert_eq!(drop_shares(&d, Some("nope")).unwrap_err(), "no such share");
    let (left, went) = drop_shares(&d, Some("a")).unwrap(); assert!(left.is_empty() && went);
    assert!(!d.join("shares-v1.json.tmp").exists());
    // a store that cannot be written: the revoke says so and the share stays
    mutate(&d, |v| v.push(sh.clone())).unwrap();
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&*d, std::fs::Permissions::from_mode(0o500)).unwrap();
        let r = drop_shares(&d, Some("a"));
        std::fs::set_permissions(&*d, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(r.unwrap_err().starts_with("could not save the shares"), "reported");
        assert_eq!(load(&d).len(), 1, "not revoked on disk, and not said to be");
    }
}

#[test] fn refusals_carry_the_same_guards_as_answers() {
    let r = String::from_utf8(refusal("403 Forbidden", "that file is not shared")).unwrap();
    for need in ["X-Content-Type-Options: nosniff", "sandbox", "Cross-Origin-Resource-Policy: same-origin", "Referrer-Policy: no-referrer", "no-store", "Content-Length: 23"] { assert!(r.contains(need), "{need}: {r}") }
}
fn s(p: &Path) -> String { p.to_string_lossy().into_owned() }

#[test] fn a_file_under_a_root_is_served_and_nothing_else_is() {
    let d = tmp("rule"); let (proj, home, out) = (d.join("proj"), d.join("home"), d.join("out"));
    for x in [&proj, &home, &out, &proj.join("mfg"), &proj.join(".git"), &proj.join(".backplane"), &proj.join("sub/.ssh"), &home.join("attachments"), &home.join("secrets")] { std::fs::create_dir_all(x).unwrap() }
    let w = |p: &Path| std::fs::write(p, b"x").unwrap();
    for f in ["mfg/board.zip", "a.step", ".git/config", ".env", ".env.local", "id_rsa", "id_ed25519.pub", "server.pem", "k.key", "sub/.ssh/known_hosts", ".backplane/review.md", ".hidden", "credentials.json", ".backplane.json"] { w(&proj.join(f)) }
    w(&out.join("x.zip")); w(&home.join("attachments/a.png")); w(&home.join("secrets/vault.json")); w(&home.join("token"));
    let roots = vec![proj.clone(), home.join("attachments")];
    let ok = |p: &str| check(&roots, &home, p).map(|(r, _)| r);
    assert_eq!(ok(&s(&proj.join("mfg/board.zip"))).unwrap(), proj.join("mfg/board.zip"));
    assert!(ok(&s(&proj.join("a.step"))).is_ok() && ok(&s(&proj.join(".backplane/review.md"))).is_ok() && ok(&s(&proj.join(".backplane.json"))).is_ok());
    assert!(ok(&s(&home.join("attachments/a.png"))).is_ok(), "the hub's attachments are the person's own files");
    for bad in [".git/config", ".env", ".env.local", "id_rsa", "id_ed25519.pub", "server.pem", "k.key", "sub/.ssh/known_hosts", ".hidden", "credentials.json"] {
        assert_eq!(ok(&s(&proj.join(bad))).unwrap_err(), "that file is not shared", "{bad}");
    }
    assert_eq!(ok(&s(&out.join("x.zip"))).unwrap_err(), "that file is not in a project");
    assert_eq!(ok(&s(&home.join("secrets/vault.json"))).unwrap_err(), "that file is not shared", "a vault, even were its home a root");
    assert_eq!(check(&[home.clone()], &home, &s(&home.join("token"))).unwrap_err(), "that file is not shared", "nothing of the hub's home but its two folders");
    assert_eq!(ok(&s(&proj.join("mfg"))).unwrap_err(), "not a file");
    assert_eq!(ok(&s(&proj.join("none.txt"))).unwrap_err(), "no such file");
    for bad in ["mfg/board.zip", "", "/", &format!("{}/mfg/../a.step", s(&proj)), &format!("{}/./a.step", s(&proj)), "/etc/pass\0wd"] { assert!(ok(bad).is_err(), "{bad:?}") }
    assert!(ok(&format!("file://{}", s(&proj.join("a.step")))).is_ok());
    // a link inside the project to a file outside it is the file outside it
    crate::plat::fs::symlink(out.join("x.zip"), proj.join("link.zip")).unwrap();
    assert_eq!(ok(&s(&proj.join("link.zip"))).unwrap_err(), "that file is not in a project");
    crate::plat::fs::symlink(&out, proj.join("outdir")).unwrap();
    assert!(ok(&s(&proj.join("outdir/x.zip"))).is_err());
    // a link to a secret is a secret
    crate::plat::fs::symlink(proj.join(".env"), proj.join("env.txt")).unwrap();
    assert!(ok(&s(&proj.join("env.txt"))).is_err());
}

#[test] fn roots_are_live_projects_and_worktrees_never_the_home_or_slash() {
    let d = tmp("roots"); let home = d.join("home"); std::fs::create_dir_all(home.join("attachments")).unwrap();
    let (a, b, wt) = (d.join("a"), d.join("b"), d.join("wt")); for x in [&a, &b, &wt] { std::fs::create_dir_all(x).unwrap() }
    let mut m = crate::model::Model::default();
    m.projects.insert("p1".into(), crate::model::ProjectMeta { title: "a".into(), root: s(&a), removed: false });
    m.projects.insert("p2".into(), crate::model::ProjectMeta { title: "b".into(), root: s(&b), removed: true });
    m.projects.insert("p3".into(), crate::model::ProjectMeta { title: "h".into(), root: crate::plat::dirs::home_or_empty(), removed: false });
    m.projects.insert("p4".into(), crate::model::ProjectMeta { title: "r".into(), root: "/".into(), removed: false });
    let mut t = crate::model::ThreadMeta::default(); t.project = "p1".into(); t.worktree = s(&wt); m.threads.insert("t1".into(), t);
    let r = roots_of(&m, &home, "");
    assert!(r.contains(&a) && r.contains(&wt) && r.contains(&home.join("attachments")), "{r:?}");
    assert!(!r.contains(&b), "a removed project");
    assert!(!r.iter().any(|x| x.parent().is_none()) && !r.iter().any(|x| s(x) == crate::plat::dirs::home_or_empty()), "never / or the whole home: {r:?}");
}

#[test] fn the_paths_a_message_names() {
    let t = "Saved `/p/mfg/gerbers.zip` and /p/mech/case.step.\nSee /p/src/main.rs:42 and (~/x/bom.csv), also https://x.io/a and relative/b.txt and /p/dir/";
    assert_eq!(candidates(t), vec!["/p/mfg/gerbers.zip", "/p/mech/case.step", "/p/src/main.rs", "~/x/bom.csv"]);
    assert!(candidates("file:///p/a.zip, done").contains(&"/p/a.zip".to_string()));
}

#[test] fn ranges_resume_only_the_same_file() {
    assert_eq!(plan("bytes=10-", "", "\"t\"", 100), Ok(("206 Partial Content", 10, 99)));
    assert_eq!(plan("bytes=10-", "\"t\"", "\"t\"", 100), Ok(("206 Partial Content", 10, 99)), "If-Range of this file: the range");
    assert_eq!(plan("bytes=10-", "\"old\"", "\"t\"", 100), Ok(("200 OK", 0, 99)), "the file changed: all of it again");
    assert_eq!(plan("", "", "\"t\"", 100), Ok(("200 OK", 0, 99)));
    assert_eq!(plan("bytes=200-", "", "\"t\"", 100), Err(()));
}

#[test] fn lives_and_names() {
    assert_eq!(life(&Value::Null), Ok(DEFAULT_LIFE_S)); assert_eq!(life(&json!("30m")), Ok(1800)); assert_eq!(life(&json!("12h")), Ok(43200));
    assert_eq!(life(&json!("7d")), Ok(7 * 86400)); assert_eq!(life(&json!("400d")), Ok(MAX_LIFE_S)); assert_eq!(life(&json!(90)), Ok(90));
    for bad in [json!("0h"), json!("soon"), json!(0), json!(-1), json!(true)] { assert!(life(&bad).is_err(), "{bad}") }
    assert_eq!(share_route("ab", "my board.zip"), "/s/ab/my%20board.zip");
    let d = disposition("ä \"x\".zip", false);
    assert!(d.starts_with("Content-Disposition: attachment; filename=\"_ _x_.zip\"; filename*=UTF-8''%C3%A4%20%22x%22.zip"), "{d}");
    assert!(!d.contains('\n') || d.ends_with("\r\n"));
    assert_eq!(ctype("A.STEP"), "model/step"); assert_eq!(ctype("x.unknown"), "application/octet-stream");
    for n in [".env", "id_rsa", "x.pem", "vault.json", "secrets.yaml", "token", "settings.local.json"] { assert!(secret_name(n), "{n}") }
    for n in ["board.kicad_pcb", "bom.csv", "tokenizer.rs", "keyboard.step"] { assert!(!secret_name(n), "{n}") }
}

#[tokio::test] async fn a_stream_is_the_range_asked_and_head_is_no_body() {
    let d = tmp("stream"); let f = d.join("big.bin");
    let data: Vec<u8> = (0..(3 * BUF + 17)).map(|i| (i % 251) as u8).collect(); std::fs::write(&f, &data).unwrap();
    let md = std::fs::metadata(&f).unwrap();
    let get = |hs: Vec<(&str, String)>, method: &str| { let mut r = Req::new(method, "/file?path=x", hs.into_iter().map(|(a, b)| (a.to_string(), b)).collect()); r.peer = None; r };
    let run = |r: Req| { let f = f.clone(); async move { let mut out: Vec<u8> = vec![]; let o = open_real(&[f.parent().unwrap().to_path_buf()], Path::new("/nonexistent-home"), &f).unwrap(); stream(&mut out, Who::Native, &r, o, "big.bin").await.unwrap(); out } };
    let split = |v: &[u8]| { let i = v.windows(4).position(|w| w == b"\r\n\r\n").unwrap(); (String::from_utf8_lossy(&v[..i]).into_owned(), v[i + 4..].to_vec()) };
    let (h, b) = split(&run(get(vec![], "GET")).await);
    assert!(h.starts_with("HTTP/1.1 200 OK") && h.contains(&format!("Content-Length: {}", data.len())) && h.contains("Accept-Ranges: bytes") && h.contains("ETag: \"") && h.contains("attachment; filename=\"big.bin\""), "{h}");
    assert_eq!(b, data);
    let tag = etag(&md);
    let (h, b) = split(&run(get(vec![("Range", format!("bytes={}-", BUF + 5)), ("If-Range", tag.clone())], "GET")).await);
    assert!(h.starts_with("HTTP/1.1 206") && h.contains(&format!("Content-Range: bytes {}-{}/{}", BUF + 5, data.len() - 1, data.len())), "{h}");
    assert_eq!(b, data[BUF + 5..]);
    let (h, b) = split(&run(get(vec![("Range", "bytes=0-9".into()), ("If-Range", "\"other\"".into())], "GET")).await);
    assert!(h.starts_with("HTTP/1.1 200") && b == data, "a changed file is sent whole");
    let (h, b) = split(&run(get(vec![], "HEAD")).await); assert!(h.starts_with("HTTP/1.1 200") && b.is_empty());
    let out = run(get(vec![("Range", "bytes=999999-".into())], "GET")).await; assert!(out.starts_with(b"HTTP/1.1 416"));
}

#[test] fn shares_are_kept_privately_and_listed_newest_first() {
    let d = tmp("store");
    let sh = |id: &str, at: u64| Share { id: id.into(), token: "t".repeat(48), path: "/p/a.zip".into(), name: "a.zip".into(), size: 1, thread: "t1".into(), created: at, expires: at + 10, tailnet: false, url: String::new() };
    save(&d, &[sh("a", 1), sh("b", 2)]).unwrap();
    assert_eq!(load(&d).len(), 2);
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; assert_eq!(std::fs::metadata(store_path(&d)).unwrap().permissions().mode() & 0o777, 0o600) }
    let l = listed(&load(&d)); assert_eq!((l[0]["id"].as_str(), l[1]["id"].as_str()), (Some("b"), Some("a")));
    assert_eq!(l[0]["link"], format!("/s/{}/a.zip", "t".repeat(48)));
}

#[test] fn headers_never_let_a_file_act_with_the_origin_and_never_leak_a_share() {
    for who in [Who::Native, Who::Browser, Who::Share] {
        let h = String::from_utf8(head_bytes(who, "200 OK", "text/plain; charset=utf-8", 1, "")).unwrap();
        for need in ["X-Content-Type-Options: nosniff", "sandbox", "Cross-Origin-Resource-Policy: same-origin", "Referrer-Policy: no-referrer", "default-src 'none'"] { assert!(h.contains(need), "{who:?} {need}") }
        assert!(h.contains(if who == Who::Native { "Cache-Control: private, no-cache" } else { "Cache-Control: private, no-store" }), "{who:?}");
    }
    for t in ["image/png", "application/pdf", "text/plain; charset=utf-8"] { assert!(inline_ok(t)) }
    for t in ["text/html", "image/svg+xml", "text/csv; charset=utf-8", "application/json", "application/octet-stream", "application/xhtml+xml"] { assert!(!inline_ok(t), "{t}") }
    for n in ["a.html", "a.htm", "a.svg", "a.xhtml", "a.js"] { assert!(!inline_ok(ctype(n)), "{n}: {}", ctype(n)) }
}
