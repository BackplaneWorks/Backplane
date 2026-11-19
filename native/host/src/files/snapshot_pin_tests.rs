//! What a snapshot, a restore and a checkpoint diff make, pinned: the refs each makes and every tree's entries (content hashes, so the same
//! on every system and every run; commits carry times and are not pinned). Written against the sh-run git (`bp_safe` in sh) before git was
//! started directly with the filter check done in Rust, so the change is held to the same refs, trees and diff.
use super::*;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new("git").arg("-C").arg(dir).args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", "-c", "core.autocrlf=false", "-c", "init.defaultBranch=main"]).args(args).output().unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).trim_end().to_string()
}
fn put(p: &Path, text: &[u8]) { std::fs::create_dir_all(p.parent().unwrap()).unwrap(); std::fs::write(p, text).unwrap() }
/// Every ref under refs/backplane/ (and HEAD's tree), each with its tree's entries: `<ref> <tree>` then `  <mode> <type> <id>\t<path>`.
fn refs_and_trees(top: &Path) -> String {
    let mut out = String::new();
    let refs = git(top, &["for-each-ref", "--format=%(refname)", "refs/backplane/"]);
    let mut names: Vec<&str> = refs.lines().collect(); names.sort();
    if git(top, &["rev-parse", "-q", "--verify", "HEAD"]).len() > 0 { names.insert(0, "HEAD") }
    for r in names {
        let tree = git(top, &["rev-parse", &format!("{r}^{{tree}}")]);
        out += &format!("{r} {tree}\n");
        for l in git(top, &["ls-tree", "-r", "--full-tree", &tree]).lines() { out += &format!("  {l}\n") }
    }
    out
}
fn rt() -> tokio::runtime::Runtime { tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap() }

/// A: a folder Backplane makes a repository (on the branch), files with spaces and non-ASCII in their paths; two turns' snapshots, a restore
/// to the first, and a checkpoint diff.
#[test] fn a_made_repository_snapshots_restores_and_diffs_as_pinned() {
    let d = crate::plat::tmp::dir("pin-a"); let root = d.join("Board Ünïcode");
    put(&root.join("a.txt"), b"one\n"); put(&root.join("dir with space/ü-naïve.txt"), b"x\n"); put(&root.join("hw/b.kicad_sch"), b"(kicad_sch)\n");
    let r = root.to_str().unwrap().to_string();
    let got = rt().block_on(async {
        let e = ensure_repo(&r, None, false).await.unwrap(); assert!(e.made, "{e:?}");
        let top = e.top.clone();
        snapshot(&top, "t1", &snap_ref("t1", 1), "turn 1", false).await.unwrap();
        put(&root.join("a.txt"), b"two\n"); put(&root.join("new file ü.txt"), b"n\n"); std::fs::remove_file(root.join("hw/b.kicad_sch")).unwrap();
        snapshot(&top, "t1", &snap_ref("t1", 2), "turn 2", false).await.unwrap();
        restore(&top, "t1", &snap_ref("t1", 1), "turn 1", false).await.unwrap();
        git::checkpoint_capture(&top, "t1", 0).await.unwrap(); put(&root.join("a.txt"), b"three\n");
        let diff = git::diff(&top, "t1", "0", "-1").await;
        (refs_and_trees(Path::new(&top)), diff, std::fs::read_to_string(root.join("a.txt")).unwrap(), root.join("new file ü.txt").exists())
    });
    assert_eq!((got.2.as_str(), got.3), ("three\n", false), "restored to turn 1, then a.txt edited");
    assert!(got.1.contains("-one") && got.1.contains("+three") && got.1.contains("a.txt"), "{}", got.1);
    assert_eq!(got.0, PIN_A, "refs and trees:\n{}", got.0);
}

/// B: the person's own repository (not Backplane's: snapshots on refs, the branch untouched) with a clean filter of its own, one more through
/// an includeIf, and core.autocrlf on with a CRLF file. A snapshot holds the files as they are: no repository filter runs (bp_safe).
#[test] fn a_persons_repository_with_filters_includeif_and_autocrlf_snapshots_as_pinned() {
    let d = crate::plat::tmp::dir("pin-b"); let root = d.join("repo");
    put(&root.join("a.txt"), b"lower\n"); put(&root.join("crlf.txt"), b"one\r\ntwo\r\n"); put(&root.join("x.up2"), b"also lower\n");
    put(&root.join(".gitattributes"), b"*.txt filter=up\n*.up2 filter=up2\n");
    git(&root, &["init", "-q"]); git(&root, &["add", "-A"]); git(&root, &["commit", "-qm", "base"]);
    // filters that would change what is stored, if run: they must not be
    git(&root, &["config", "filter.up.clean", "tr a-z A-Z"]); git(&root, &["config", "filter.up.smudge", "cat"]);
    put(&root.join(".git/extra.cfg"), b"[filter \"up2\"]\n\tclean = tr a-z A-Z\n");
    let gd = root.join(".git").to_string_lossy().replace('\\', "/");
    git(&root, &["config", &format!("includeIf.gitdir/i:{gd}.path"), "extra.cfg"]);
    git(&root, &["config", "core.autocrlf", "true"]);
    put(&root.join("a.txt"), b"lower, changed\n");
    let r = root.to_str().unwrap().to_string();
    let got = rt().block_on(async {
        let e = ensure_repo(&r, None, false).await.unwrap(); assert!(!e.made);
        snapshot(&e.top, "t2", &snap_ref("t2", 1), "turn 1", false).await.unwrap();
        put(&root.join("crlf.txt"), b"one\r\ntwo\r\nthree\r\n");
        snapshot(&e.top, "t2", &snap_ref("t2", 2), "turn 2", false).await.unwrap();
        restore(&e.top, "t2", &snap_ref("t2", 1), "turn 1", false).await.unwrap();
        refs_and_trees(Path::new(&e.top))
    });
    // stored as the files are, no filter run: the local one (a.txt) nor the one an includeIf brings (x.up2); the CRLF file as autocrlf leaves
    // a file the index already holds with CRLF
    for (path, raw) in [("a.txt", &b"lower, changed\n"[..]), ("x.up2", b"also lower\n"), ("crlf.txt", b"one\r\ntwo\r\n")] {
        let o = Command::new("git").args(["hash-object", "--stdin", "--no-filters"]).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
        { use std::io::Write; o.stdin.as_ref().unwrap().write_all(raw).unwrap(); } let id = String::from_utf8(o.wait_with_output().unwrap().stdout).unwrap();
        assert!(got.contains(&format!("{}\t{path}\n", id.trim())), "{path} stored raw: {got}");
    }
    assert_eq!(std::fs::read(root.join("crlf.txt")).unwrap(), b"one\r\ntwo\r\n", "restored, with the line ends autocrlf gives back here");
    assert_eq!(got, PIN_B, "refs and trees:\n{got}");
}

/// C: a repository with another inside it (not a submodule): a snapshot of the outer one, and one taken from inside the inner one.
#[test] fn a_nested_repository_snapshots_as_pinned() {
    let d = crate::plat::tmp::dir("pin-c"); let outer = d.join("outer"); let inner = outer.join("lib/inner");
    put(&outer.join("o.txt"), b"outer\n"); put(&inner.join("i.txt"), b"inner\n");
    git(&outer, &["init", "-q"]); git(&inner, &["init", "-q"]); git(&inner, &["add", "-A"]); git(&inner, &["commit", "-qm", "inner"]);
    git(&outer, &["add", "o.txt"]); git(&outer, &["commit", "-qm", "outer"]);
    let (o, i) = (outer.to_str().unwrap().to_string(), inner.to_str().unwrap().to_string());
    let got = rt().block_on(async {
        let eo = ensure_repo(&o, None, false).await.unwrap(); snapshot(&eo.top, "t3", &snap_ref("t3", 1), "outer", false).await.unwrap();
        let ei = ensure_repo(&i, None, false).await.unwrap(); put(&inner.join("i.txt"), b"inner, changed\n");
        snapshot(&ei.top, "t3", &snap_ref("t3", 1), "inner", false).await.unwrap();
        (refs_and_trees(Path::new(&eo.top)), refs_and_trees(Path::new(&ei.top)))
    });
    // the inner repository's commit id is in the outer tree (a gitlink): it carries a time, so it is checked, not pinned
    let inner_head = git(&inner, &["rev-parse", "HEAD"]); let snap_tree = git(&outer, &["rev-parse", &format!("{}^{{tree}}", snap_ref("t3", 1))]);
    assert_eq!(got.0.replace(&inner_head, "<inner HEAD>").replace(&snap_tree, "<its tree>"), PIN_C_OUTER, "outer:\n{}", got.0);
    assert_eq!(got.1, PIN_C_INNER, "inner:\n{}", got.1);
}

const PIN_A: &str = r#"HEAD cc394f6456ccb561146690c7de55204c8ba38b07
  100644 blob c10226d163aa7242d8f5d63ec8b6c12670905e9c	.gitignore
  100644 blob 5626abf0f72e58d7a153368ba57db4c673c0e171	a.txt
  100644 blob 587be6b4c3f93f93c489c0111bba5596147a26cb	"dir with space/\303\274-na\303\257ve.txt"
  100644 blob b5779c7be14a675c4ea6bdbf76c22e088a65cf2d	hw/b.kicad_sch
refs/backplane/checkpoints/dDE/turn/0 cc394f6456ccb561146690c7de55204c8ba38b07
  100644 blob c10226d163aa7242d8f5d63ec8b6c12670905e9c	.gitignore
  100644 blob 5626abf0f72e58d7a153368ba57db4c673c0e171	a.txt
  100644 blob 587be6b4c3f93f93c489c0111bba5596147a26cb	"dir with space/\303\274-na\303\257ve.txt"
  100644 blob b5779c7be14a675c4ea6bdbf76c22e088a65cf2d	hw/b.kicad_sch
refs/backplane/snapshots/dDE/1 cc394f6456ccb561146690c7de55204c8ba38b07
  100644 blob c10226d163aa7242d8f5d63ec8b6c12670905e9c	.gitignore
  100644 blob 5626abf0f72e58d7a153368ba57db4c673c0e171	a.txt
  100644 blob 587be6b4c3f93f93c489c0111bba5596147a26cb	"dir with space/\303\274-na\303\257ve.txt"
  100644 blob b5779c7be14a675c4ea6bdbf76c22e088a65cf2d	hw/b.kicad_sch
refs/backplane/snapshots/dDE/1-before-restore 7a9fc723a906d9c1cc7a465944fa25f742f6a6d7
  100644 blob c10226d163aa7242d8f5d63ec8b6c12670905e9c	.gitignore
  100644 blob f719efd430d52bcfc8566a43b2eb655688d38871	a.txt
  100644 blob 587be6b4c3f93f93c489c0111bba5596147a26cb	"dir with space/\303\274-na\303\257ve.txt"
  100644 blob 8ba3a16384aacc37d01564b28401755ce8053f51	"new file \303\274.txt"
refs/backplane/snapshots/dDE/1-restored cc394f6456ccb561146690c7de55204c8ba38b07
  100644 blob c10226d163aa7242d8f5d63ec8b6c12670905e9c	.gitignore
  100644 blob 5626abf0f72e58d7a153368ba57db4c673c0e171	a.txt
  100644 blob 587be6b4c3f93f93c489c0111bba5596147a26cb	"dir with space/\303\274-na\303\257ve.txt"
  100644 blob b5779c7be14a675c4ea6bdbf76c22e088a65cf2d	hw/b.kicad_sch
refs/backplane/snapshots/dDE/2 7a9fc723a906d9c1cc7a465944fa25f742f6a6d7
  100644 blob c10226d163aa7242d8f5d63ec8b6c12670905e9c	.gitignore
  100644 blob f719efd430d52bcfc8566a43b2eb655688d38871	a.txt
  100644 blob 587be6b4c3f93f93c489c0111bba5596147a26cb	"dir with space/\303\274-na\303\257ve.txt"
  100644 blob 8ba3a16384aacc37d01564b28401755ce8053f51	"new file \303\274.txt"
"#;
const PIN_B: &str = r#"HEAD f251176a49f3bdc1bcd73f329243b756a0c6710b
  100644 blob b9856817ff1a17d09bb13dc4486693a267dbc4bf	.gitattributes
  100644 blob 3f2d9cedc75ad2766b40a4158022fcc9f5dfe9b1	a.txt
  100644 blob 4e349b596c5c9d38a82829fafbaf52281c21e319	crlf.txt
  100644 blob ee1717029052f56267b8731ce6bd1dfab74e4bc2	x.up2
refs/backplane/snapshots/dDI/1 d3d6c30899e131c7a2e38bb37fdc6d8c35ffca34
  100644 blob b9856817ff1a17d09bb13dc4486693a267dbc4bf	.gitattributes
  100644 blob 34ede5bc739171c7712e15ae01af5ffe4cca7cdc	a.txt
  100644 blob 4e349b596c5c9d38a82829fafbaf52281c21e319	crlf.txt
  100644 blob ee1717029052f56267b8731ce6bd1dfab74e4bc2	x.up2
refs/backplane/snapshots/dDI/1-before-restore 65ddd866086b83896e97903b2d0ebf0cfb0fbb0e
  100644 blob b9856817ff1a17d09bb13dc4486693a267dbc4bf	.gitattributes
  100644 blob 34ede5bc739171c7712e15ae01af5ffe4cca7cdc	a.txt
  100644 blob e1587ff9ea6ced3af32e07e4b6b68d21374907ec	crlf.txt
  100644 blob ee1717029052f56267b8731ce6bd1dfab74e4bc2	x.up2
refs/backplane/snapshots/dDI/1-restored d3d6c30899e131c7a2e38bb37fdc6d8c35ffca34
  100644 blob b9856817ff1a17d09bb13dc4486693a267dbc4bf	.gitattributes
  100644 blob 34ede5bc739171c7712e15ae01af5ffe4cca7cdc	a.txt
  100644 blob 4e349b596c5c9d38a82829fafbaf52281c21e319	crlf.txt
  100644 blob ee1717029052f56267b8731ce6bd1dfab74e4bc2	x.up2
refs/backplane/snapshots/dDI/2 65ddd866086b83896e97903b2d0ebf0cfb0fbb0e
  100644 blob b9856817ff1a17d09bb13dc4486693a267dbc4bf	.gitattributes
  100644 blob 34ede5bc739171c7712e15ae01af5ffe4cca7cdc	a.txt
  100644 blob e1587ff9ea6ced3af32e07e4b6b68d21374907ec	crlf.txt
  100644 blob ee1717029052f56267b8731ce6bd1dfab74e4bc2	x.up2
"#;
const PIN_C_OUTER: &str = r#"HEAD 3069c63489caf167b163b25493f2f9b5ceed631d
  100644 blob 6d2abcd5423abda612156294d01636f0abdb831e	o.txt
refs/backplane/snapshots/dDM/1 <its tree>
  160000 commit <inner HEAD>	lib/inner
  100644 blob 6d2abcd5423abda612156294d01636f0abdb831e	o.txt
"#;
const PIN_C_INNER: &str = r#"HEAD f00aa2d0f6fa3ab541d938b5dd9d8961769e5252
  100644 blob f05648e753bc95da97c2b753903c1111061d67af	i.txt
refs/backplane/snapshots/dDM/1 dced247df3db542b4bcf011abbb58e68018c3d69
  100644 blob 1f41d3c05f2335d36be5587ce0ad69c2a4224456	i.txt
"#;

/// The time a turn waits for its snapshot (what snapguard.rs `take_snapshot` runs: the repository made or found, ownership, the next turn,
/// the snapshot), for a fresh folder of 30 small files and then for five later turns with one file changed each. Printed (`SNAPTIME`), not
/// bounded: it is the measure the direct git was made for (Windows, Nov 19).
#[test] fn a_turns_snapshot_is_timed() {
    let d = crate::plat::tmp::dir("pin-time"); let root = d.join("proj");
    for i in 0..30 { put(&root.join(format!("f{i:02}.txt")), format!("file {i}\n").as_bytes()) }
    let r = root.to_str().unwrap().to_string();
    rt().block_on(async {
        let turn = |n: u64| { let r = r.clone(); async move {
            let t = std::time::Instant::now();
            let e = ensure_repo(&r, None, false).await.unwrap(); let _ = owned(&e.top).await; let k = next_turn(&e.top, "tt").await; assert_eq!(k, n);
            snapshot(&e.top, "tt", &snap_ref("tt", k), "turn", false).await.unwrap(); t.elapsed() } };
        let first = turn(1).await; let mut later = vec![];
        for n in 2..=6 { put(&root.join(format!("f{:02}.txt", n)), format!("changed {n}\n").as_bytes()); later.push(turn(n).await) }
        let avg = later.iter().sum::<std::time::Duration>() / later.len() as u32;
        eprintln!("SNAPTIME first turn {:?}; later turns {:?} (mean {:?})", first, later, avg);
    });
}
