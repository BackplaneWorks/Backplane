//! Policy only. The client continues to choose its surface until the rollout.
use serde_json::{json, Value};

#[derive(Clone, Copy, PartialEq)]
pub enum Platform { Linux, Mac, Windows, Other }

impl Platform {
    pub fn host() -> Self {
        match std::env::consts::OS { "linux" => Self::Linux, "macos" => Self::Mac, "windows" => Self::Windows, _ => Self::Other }
    }
}

// Linux and macOS after PR #1348 and the runtime fields land. Windows after issues #1343 and #1344.
pub const AUTO_PLATFORMS: &[Platform] = &[];

#[derive(Clone, Copy, PartialEq)]
pub enum Preference { Auto, Viewer, Kicad }

pub const SETTING: &str = "viewer.designMode";
impl Preference {
    pub fn read(value: &str) -> Self {
        match value { "viewer" => Self::Viewer, "kicad" => Self::Kicad, _ => Self::Auto }
    }
}

#[derive(Clone, Copy)]
pub enum Document { Present, Missing, Unknown, OtherPage }
#[derive(Clone, Copy)]
pub enum Visit { Normal, OpenKicad, ShowViewer }
#[derive(Clone, Copy)]
pub enum Support { Unknown, Missing, Unsupported, Supported }
#[derive(Clone, Copy)]
pub enum Session { Unknown, Cold, Starting, Live, Failed, IdleStopped, ClosedByPerson }
#[derive(Clone, Copy)]
pub enum WarmBlock { None, Memory, Capacity, ExternalLock }

#[derive(Clone, Copy)]
pub struct Runtime {
    pub support: Support, pub session: Session, pub warm_block: WarmBlock,
    // Identifies the lifetime of the reported session for the future attach reconciler.
    pub generation: Option<u64>, pub holder: Option<u64>,
}
impl Runtime {
    pub const UNKNOWN: Self = Self { support: Support::Unknown, session: Session::Unknown,
        warm_block: WarmBlock::None, generation: None, holder: None };
}

#[derive(Clone, Copy)]
pub struct Input {
    /// A local native desktop connection's hub-assigned ID. Others cannot host native frames.
    pub desktop: Option<u64>, pub linked: bool, pub altium: bool, pub document: Document,
    pub platform: Platform, pub preference: Preference, pub visit: Visit, pub runtime: Runtime,
}

/// Board and Schematic share a surface policy. Moving between them switches frames in the same project.
/// Focus and the cold-start dwell belong to the client. Automatic attach leaves keyboard focus alone.
/// A generation identifies a runtime lifetime. Presenter updates, including a new generation, never retry a
/// failed start. OpenKicad is a deliberate retry and overrides Always viewer for the current visit.
pub fn design_mode(input: Input, rollout: &[Platform]) -> Value {
    use {Session::*, Visit::*};
    let viewer = |why| json!({"want":"viewer", "why":why});
    let r = input.runtime;
    if input.linked { return viewer("linkedThread") }
    let Some(client) = input.desktop else { return viewer("remoteClient") };
    if input.altium { return viewer("altium") }
    match input.document { Document::OtherPage => return viewer("page"), Document::Missing => return viewer("missingDocument"), Document::Unknown => return viewer("documentUnknown"), Document::Present => {} }
    if matches!(input.visit, ShowViewer) { return viewer("thisVisit") }
    let deliberate = matches!(input.visit, OpenKicad);
    if input.preference == Preference::Viewer && !deliberate { return viewer("preference") }
    match r.support {
        Support::Unknown => return viewer("runtimeUnknown"), Support::Missing => return viewer("runtimeMissing"),
        Support::Unsupported => return viewer("unsupported"), Support::Supported => {}
    }
    match r.session {
        Unknown => return viewer("runtimeUnknown"),
        ClosedByPerson if !deliberate => return viewer("closedByPerson"),
        IdleStopped if !deliberate => return viewer("idleStopped"),
        Failed if !deliberate => return viewer("failed"),
        _ => {}
    }
    let requested = deliberate || input.preference == Preference::Kicad;
    if !requested && !rollout.contains(&input.platform) { return viewer("rollout") }
    if !matches!(r.session, Live | Starting) {
        match r.warm_block {
            WarmBlock::ExternalLock => return viewer("externalLock"), WarmBlock::Capacity => return viewer("capacity"),
            WarmBlock::Memory if !requested => return viewer("memory"), _ => {}
        }
    }
    if r.holder.is_some_and(|holder| holder != client) { return viewer("anotherWindow") }
    json!({"want":"kicad","why": if deliberate { "thisVisit" } else if requested { "preference" } else { "auto" }})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mode_truth_table() {
        let runtime = Runtime { support: Support::Supported, session: Session::Cold, ..Runtime::UNKNOWN };
        let base = Input { desktop: Some(4), linked: false, altium: false, document: Document::Present,
            platform: Platform::Linux, preference: Preference::Auto, visit: Visit::Normal, runtime };
        let mode = |i| design_mode(i, &[Platform::Linux, Platform::Mac]);
        let v = |why| json!({"want":"viewer","why":why});
        let k = |why| json!({"want":"kicad","why":why});
        for (i, result) in [
            (base, k("auto")), (Input { platform:Platform::Mac,..base }, k("auto")),
            (Input { platform:Platform::Windows,..base }, v("rollout")),
            (Input { desktop:None,..base }, v("remoteClient")), (Input { linked:true,..base }, v("linkedThread")),
            (Input { altium:true,..base }, v("altium")), (Input { document:Document::Missing,..base }, v("missingDocument")),
            (Input { document:Document::OtherPage,..base }, v("page")),
            (Input { preference:Preference::Viewer,..base }, v("preference")),
            (Input { visit:Visit::ShowViewer,preference:Preference::Kicad,..base }, v("thisVisit")),
            (Input { runtime:Runtime::UNKNOWN, preference:Preference::Kicad,..base }, v("runtimeUnknown")),
        ] { assert_eq!(mode(i), result); }
        for (support, why) in [(Support::Missing,"runtimeMissing"),(Support::Unsupported,"unsupported")] {
            assert_eq!(mode(Input { runtime:Runtime { support,..runtime },..base }), v(why));
        }
        for (session, why) in [(Session::Failed,"failed"),(Session::IdleStopped,"idleStopped"),(Session::ClosedByPerson,"closedByPerson")] {
            let i = Input { runtime:Runtime { session,..runtime }, preference:Preference::Kicad,..base };
            assert_eq!(mode(i), v(why)); assert_eq!(mode(Input {visit:Visit::OpenKicad,..i}), k("thisVisit"));
        }
        for (warm_block, why) in [(WarmBlock::Capacity,"capacity"),(WarmBlock::ExternalLock,"externalLock"),(WarmBlock::Memory,"memory")] {
            let i = Input {runtime:Runtime {warm_block,..runtime},..base};
            assert_eq!(mode(i), v(why));
            for (preference, visit, reason) in [(Preference::Kicad, Visit::Normal,"preference"),(Preference::Viewer,Visit::OpenKicad,"thisVisit")] {
                assert_eq!(mode(Input {preference,visit,..i}), if why=="memory" {k(reason)} else {v(why)});
            }
        }
        for session in [Session::Starting, Session::Live] {
            let live = Runtime {session, generation:Some(1),warm_block:WarmBlock::Memory,..runtime};
            assert_eq!(mode(Input {runtime:live,..base}), k("auto"));
            assert_eq!(mode(Input {runtime:Runtime {holder:Some(8),..live},..base}), v("anotherWindow"));
            assert_eq!(mode(Input {runtime:Runtime {holder:Some(4),..live},..base}), k("auto"));
        }
        assert_eq!(mode(Input { platform: Platform::Windows, preference: Preference::Kicad, ..base }), k("preference"));
        for (i, why) in [
            (Input { desktop: None, ..base }, "remoteClient"), (Input { linked: true, ..base }, "linkedThread"),
            (Input { altium: true, ..base }, "altium"), (Input { document: Document::OtherPage, ..base }, "page"),
            (Input { document: Document::Missing, ..base }, "missingDocument"),
            (Input { document: Document::Unknown, ..base }, "documentUnknown"),
        ] {
            for visit in [Visit::Normal, Visit::OpenKicad] {
                assert_eq!(mode(Input { preference: Preference::Kicad, visit, ..i }), v(why));
            }
        }
        for (session, holder) in [(Session::Cold,None), (Session::Live,None), (Session::Live,Some(4))] {
            let i = Input { preference: Preference::Viewer, runtime: Runtime { session, holder, ..runtime }, ..base };
            assert_eq!(mode(i), v("preference"));
            assert_eq!(mode(Input { visit: Visit::OpenKicad, ..i }), k("thisVisit"));
        }
        let blocked = Runtime { holder: Some(8), warm_block: WarmBlock::Capacity, ..runtime };
        for (session, why) in [(Session::Unknown,"runtimeUnknown"), (Session::Failed,"failed"),
            (Session::IdleStopped,"idleStopped"), (Session::ClosedByPerson,"closedByPerson"), (Session::Cold,"capacity")] {
            assert_eq!(mode(Input { runtime: Runtime { session, ..blocked }, ..base }), v(why));
        }
        for generation in [Some(1), Some(2)] {
            let failed = Input { runtime: Runtime { session: Session::Failed, generation, ..runtime }, ..base };
            assert_eq!(mode(failed), v("failed"));
            assert_eq!(mode(Input { visit: Visit::OpenKicad, ..failed }), k("thisVisit"));
        }
        assert_eq!(design_mode(base,AUTO_PLATFORMS),v("rollout"));
    }
}
