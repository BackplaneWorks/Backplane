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
pub enum WarmBlock { Unknown, None, Memory, Capacity, ExternalLock }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StopReason { Unknown, Idle, Evicted, AppClosed, CloseAsked, AgentClosed, KicadQuit, Crashed, StartFailed }

#[derive(Clone, Copy)]
pub enum Frame { Board, Schematic }

#[derive(Clone, Copy, Default)]
pub struct ClosedFrames { board: bool, schematic: bool }
impl ClosedFrames {
    fn contains(self, frame: Option<Frame>) -> bool {
        match frame { Some(Frame::Board) => self.board, Some(Frame::Schematic) => self.schematic, None => false }
    }
}

#[derive(Clone, Copy)]
pub struct Runtime<'a> {
    pub support: Support, pub session: Session, pub warm_block: WarmBlock,
    pub stop_reason: StopReason, pub stopped_at: Option<u64>, pub closed_frames: ClosedFrames,
    pub warm_why: &'a str, pub warm_at: Option<u64>, pub generation: Option<u64>, pub holder: Option<u64>,
}

/// Decode the owner’s runtime picture. Diagnostics are carried verbatim and never decide mode.
pub fn runtime_from_picture(p: &Value) -> Runtime<'_> {
    let field = |name: &str| p[name].as_str().unwrap_or("");
    let number = |name: &str| p[name].as_u64().or_else(|| field(name).parse().ok());
    let mut closed_frames = ClosedFrames::default();
    for frame in p["closedFrames"].as_array().into_iter().flatten() {
        match frame.as_str() { Some("PCB") => closed_frames.board = true, Some("SCH") => closed_frames.schematic = true, _ => {} }
    }
    Runtime {
        support: match field("support") { "supported" => Support::Supported, "missing" => Support::Missing,
            "unsupported" => Support::Unsupported, _ => Support::Unknown },
        session: match field("session") { "cold" => Session::Cold, "starting" => Session::Starting, "live" => Session::Live,
            "failed" => Session::Failed, "idleStopped" => Session::IdleStopped, "closedByPerson" => Session::ClosedByPerson, _ => Session::Unknown },
        stop_reason: match field("stopReason") { "idle" => StopReason::Idle, "evicted" => StopReason::Evicted,
            "appClosed" => StopReason::AppClosed, "closeAsked" => StopReason::CloseAsked, "agentClosed" => StopReason::AgentClosed,
            "kicadQuit" => StopReason::KicadQuit, "crashed" => StopReason::Crashed, "startFailed" => StopReason::StartFailed, _ => StopReason::Unknown },
        warm_block: match field("warmBlock") { "none" => WarmBlock::None, "memory" => WarmBlock::Memory,
            "capacity" => WarmBlock::Capacity, "externalLock" => WarmBlock::ExternalLock, _ => WarmBlock::Unknown },
        closed_frames, stopped_at: number("stoppedAt"), generation: number("generation"), holder: number("holder"),
        warm_why: field("warmWhy"), warm_at: number("warmAt"),
    }
}

#[derive(Clone, Copy)]
pub struct Input<'a> {
    /// A local native desktop connection's hub-assigned ID. Others cannot host native frames.
    pub desktop: Option<u64>, pub linked: bool, pub altium: bool, pub document: Document,
    pub platform: Platform, pub preference: Preference, pub visit: Visit, pub runtime: Runtime<'a>,
    pub frame: Option<Frame>, pub visit_started_after_stop: bool,
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
    let session = match r.session {
        Live if r.closed_frames.contains(input.frame) => ClosedByPerson,
        IdleStopped if input.visit_started_after_stop => Cold,
        IdleStopped | ClosedByPerson if matches!(r.stop_reason, StopReason::Evicted | StopReason::AgentClosed) => Cold,
        other => other,
    };
    match session {
        Unknown => return viewer("runtimeUnknown"),
        ClosedByPerson if !deliberate => return viewer("closedByPerson"),
        IdleStopped if !deliberate => return viewer("idleStopped"),
        Failed if !deliberate => return viewer("failed"),
        _ => {}
    }
    let requested = deliberate || input.preference == Preference::Kicad;
    if !requested && !rollout.contains(&input.platform) { return viewer("rollout") }
    if !matches!(session, Live | Starting) {
        match r.warm_block {
            WarmBlock::Unknown => return viewer("runtimeUnknown"),
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
    fn runtime_picture_decodes_wire_fields_and_unknown_values() {
        let picture = json!({"support":"supported","session":"failed","stopReason":"startFailed","stoppedAt":13,
            "closedFrames":["SCH","future"],"warmBlock":"externalLock","warmWhy":"memory","warmAt":"12","generation":2,"holder":"4"});
        let r = runtime_from_picture(&picture);
        assert!(matches!((r.support,r.session,r.warm_block),(Support::Supported,Session::Failed,WarmBlock::ExternalLock)));
        assert_eq!(r.stop_reason, StopReason::StartFailed);
        assert!(r.closed_frames.schematic && !r.closed_frames.board);
        assert_eq!((r.stopped_at,r.warm_why,r.warm_at,r.generation,r.holder),(Some(13),"memory",Some(12),Some(2),Some(4)));
        let future = json!({"support":"future","session":"future","warmBlock":"future","stopReason":"future","holder":[]});
        let r = runtime_from_picture(&future);
        assert!(matches!((r.support,r.session,r.warm_block,r.stop_reason),(Support::Unknown,Session::Unknown,WarmBlock::Unknown,StopReason::Unknown)));
        assert_eq!((r.holder,r.generation),(None,None));
    }
    #[test]
    fn mode_truth_table() {
        let runtime = Runtime { support: Support::Supported, session: Session::Cold, warm_block: WarmBlock::None, ..runtime_from_picture(&Value::Null) };
        let base = Input { desktop: Some(4), linked: false, altium: false, document: Document::Present,
            platform: Platform::Linux, preference: Preference::Auto, visit: Visit::Normal, runtime, frame: Some(Frame::Board), visit_started_after_stop: false };
        fn mode(i: Input<'_>) -> Value { design_mode(i, &[Platform::Linux, Platform::Mac]) }
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
            (Input { runtime:runtime_from_picture(&Value::Null), preference:Preference::Kicad,..base }, v("runtimeUnknown")),
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
        for (closed, board_closed) in [("PCB",true), ("SCH",false)] {
            let picture = json!({"support":"supported","session":"live","closedFrames":[closed],"warmBlock":"none"});
            for (frame, is_closed) in [(Frame::Board,board_closed), (Frame::Schematic,!board_closed)] {
                let i = Input { runtime: runtime_from_picture(&picture), frame: Some(frame), ..base };
                assert_eq!(mode(i), if is_closed { v("closedByPerson") } else { k("auto") });
                assert_eq!(mode(Input { visit: Visit::OpenKicad, ..i }), k("thisVisit"));
            }
        }
        for (why, same_visit) in [("idle",v("idleStopped")), ("agentClosed",k("auto")), ("evicted",k("auto"))] {
            let picture = json!({"support":"supported","session":"idleStopped","stopReason":why,"warmBlock":"none"});
            let i = Input { runtime: runtime_from_picture(&picture), ..base };
            assert_eq!(mode(i), same_visit);
            assert_eq!(mode(Input { visit_started_after_stop:true, ..i }), k("auto"));
        }
        assert_eq!(design_mode(base,AUTO_PLATFORMS),v("rollout"));
    }
}
