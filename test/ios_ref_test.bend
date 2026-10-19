import Base
import ./check.bend as C
import ./hubs_test.bend as F
import ../src/mobile/hubs.bend as H
import ../src/core/model.bend as M
import ../src/core/json.bend as J
import ../src/core/client.bend as U
import ../src/mobile/session.bend as S
import ../src/core/refview.bend as RE

def request(+hub: String, +uri: String) -> String:
  J.Json.show(J.Json.obj([J.KV{"hub", J.Str{hub}}, J.KV{"uri", J.Str{uri}}]))

def go(h: H.Hubs, hub: String, uri: String) -> H.Hubs:
  F.act(h, "ref-open", request(hub, uri))

def duplicate() -> H.Hubs:
  H.Hubs.after(H.Hubs.recv(F.two(), "box:3787", F.changes([M.ThreadCreated{"t1", "p2", "Duplicate", M.Local{}, "", "", "claude", "", 100n}, M.ThreadCreated{"link~t1", "p2", "Remote", M.Local{}, "", "", "claude", "", 100n}, M.ThreadCreated{"space/id", "p2", "Encoded", M.Local{}, "", "", "claude", "", 100n}])))

def focus(+h: H.Hubs) -> String:
  H.Hubs.cur(h) ++ "|" ++ U.Ui.sel(H.Hubs.ui(h, H.Hubs.cur(h)))

def trail(h: H.Hubs) -> String:
  RE.Trail.cur(U.Trail.read(H.Hubs.ui(h, "box:3787")))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("canonical external known owner", focus(go(F.two(), "", "backplane://thread/t2")), "box:3787|t2")
    C.Check.str("legacy external same owner", focus(go(F.two(), "", "backplane://open?thread=t2")), "box:3787|t2")
    C.Check.str("source hub beats duplicate local id", focus(go(duplicate(), "box:3787", "backplane://thread/t1")), "box:3787|t1")
    C.Check.str("other hub preserved", F.sel(go(duplicate(), "box:3787", "backplane://thread/t1"), "100.1.1.1:3787"), "")
    C.Check.str("remote link preserved", focus(go(duplicate(), "box:3787", "backplane://thread/link~t1")), "box:3787|link~t1")
    C.Check.str("percent encoded segment", focus(go(duplicate(), "box:3787", "backplane://thread/space%2Fid")), "box:3787|space/id")
    C.Check.str("known web origin routed separately", focus(go(F.two(), "box:3787", "https://box:3787/#/thread/t2")), "box:3787|t2")
    C.Check.str("wrong source cannot fall back", focus(go(F.two(), "100.1.1.1:3787", "backplane://thread/t2")), focus(F.two()))
    C.Check.str("unknown hub cannot fall back", focus(go(F.two(), "unpaired:3787", "backplane://thread/t2")), focus(F.two()))
    C.Check.str("unresolved thread unchanged", focus(go(F.two(), "", "backplane://thread/missing")), focus(F.two()))
    C.Check.str("unknown internal unchanged", focus(go(F.two(), "", "backplane://surprise/t2")), focus(F.two()))
    C.Check.str("foreign scheme unchanged", focus(go(F.two(), "", "surprise://thread/t2")), focus(F.two()))
    C.Check.str("empty reference unchanged", focus(go(F.two(), "", "backplane://thread/")), focus(F.two()))
    C.Check.str("no pairing command", Nat.show(List.length(&2, H.HCmd, H.Hubs.cmds(H.Hubs.act(F.two(), "ref-open", request("", "backplane://surprise/t2"))))), "0")
    C.Check.str("fragment preserved in canonical history", trail(go(F.two(), "box:3787", "backplane://thread/t2#m%2F1")), "backplane://thread/t2#m%2F1")
    C.Check.str("recent session routes explicit owning hub", S.Session.target(F.two(), "100.1.1.1:3787", "ref-open", request("box:3787", "backplane://thread/t2")), "box:3787")
    C.Check.str("unknown recent hub keeps focus but rejects payload", S.Session.target(F.two(), "100.1.1.1:3787", "ref-open", request("unknown:3787", "backplane://thread/t2")), "100.1.1.1:3787")
    C.Check.str("legacy percent segment matches canonical", focus(go(duplicate(), "box:3787", "backplane://open?thread=space%2Fid")), "box:3787|space/id")
