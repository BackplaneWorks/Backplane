//! `git.off_branch` (#1188): Settings > Projects and git > Keep Backplane's commits off my branch. Off unless the person turns it on ("on").
//! Off (the default, unchanged): the hub commits the design files an agent changed onto the checked-out branch at each turn's end and save
//! (kicad/canonical.rs `commit_design`), and a repository Backplane made gets its snapshots on its branch (files/snapshot.rs).
//! On: the same commits are made, on Backplane's own refs instead (`refs/backplane/design/<branch>`, `refs/backplane/snapshots/...`,
//! `refs/backplane/first`), so every change is still recorded and /restore and the KiCad history work as before, but the person's branch,
//! index and work tree are never written: what lands on the branch, and when, is theirs. Nothing on those refs is ever pushed.
pub const KEY: &str = "git.off_branch";
/// Keep the hub's commits off the branch: only when the person turned it on.
pub fn on(v: &str) -> bool { v == "on" }

#[cfg(test)]
mod tests {
    #[test] fn off_unless_turned_on() {
        assert!(!super::on("") && !super::on("off") && super::on("on"));
    }
}
