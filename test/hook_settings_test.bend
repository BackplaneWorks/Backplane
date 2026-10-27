import Base
import ./check.bend as C
import ../src/core/client.bend as U
import ../src/core/i18n.bend as I
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/msgs.bend as MSG
import ../src/core/wnote.bend as WN
import ../src/web/view.bend as W
import ../src/web/dom.bend as D
import ../src/mobile/screen.bend as P
import ../src/app/layout.bend as L
import ../src/app/nui.bend as N

# Settings' webhook rows (client.bend's Hookv): the address and the secret
# file's path, drawn by the window, the web and the phones from one list.
# An address must be http(s) or empty. The secret's contents are never a
# field and never a label.

def saved() -> M.State:
  M.State.replay([M.SettingSet{WN.Wn.url.key(), "https://example.com/hook", 1n}, M.SettingSet{WN.Wn.secret.key(), "/tmp/hook.secret", 1n}], M.State.empty())

def ui() -> U.Ui:
  U.Ui.far.st(U.Ui.init(10n), saved())

def after(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}:
      u

def opened() -> U.Ui:
  after(U.Settings.go(ui(), "threads"))

def attrs(as: List<&2, D.Attr>) -> String:
  match as:
    case Nil{}:
      ""
    case D.At{_, v} <> t:
      v ++ " " ++ attrs(t)
    case _ <> t:
      attrs(t)

def texts(ns: List<&2, D.Node>) -> String:
  match ns:
    case Nil{}:
      ""
    case D.Txt{s} <> t:
      s ++ " " ++ texts(t)
    case D.El{_, _, as, kids} <> t:
      attrs(as) ++ texts(kids) ++ texts(t)

def rowtext(rs: List<&2, U.AgRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case U.AgRow{l, n, _, _} <> t:
      l ++ " " ++ n ++ "\n" ++ rowtext(t)

def fields(fs: List<&2, U.AgField>) -> String:
  match fs:
    case Nil{}:
      ""
    case U.AgField{n, _, sec, v, _} <> t:
      n ++ "=" ++ v ++ " secret=" ++ Bool.show(sec) ++ "\n" ++ fields(t)

def rowfields(rs: List<&2, U.AgRow>) -> String:
  match rs:
    case Nil{}:
      ""
    case U.AgRow{_, _, fs, _} <> t:
      fields(fs) ++ rowfields(t)

def hit(h: N.Hit) -> String:
  match h:
    case N.Hit{_, _, _, _, a, v}:
      a ++ ":" ++ v ++ "\n"

def hits(hs: List<&2, N.Hit>) -> String:
  match hs:
    case Nil{}:
      ""
    case h <> t:
      hit(h) ++ hits(t)

def window(o: L.Out) -> String:
  match o:
    case L.Out{_, hs}:
      hits(hs)

def cmd.sends(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}:
      ""
    case U.Send{s} <> t:
      s ++ "\n" ++ cmd.sends(t)
    case _ <> t:
      cmd.sends(t)

def sends(a: U.Act) -> String:
  match a:
    case U.Act{_, cs}:
      cmd.sends(cs)

# through Ui.act, as a click arrives: an incoming hook's action reaches
# the bot-action table and leaves the webhook's settings alone
def acted(+action: String, +value: String) -> U.Act:
  U.Ui.act(opened(), action, value)

def main() -> IO(Unit):
  do IO<Unit>:
    +u : U.Ui = opened()
    +rs : List<&2, U.AgRow> = U.Hookv.rows.loc(I.En{}, u)
    +ja : List<&2, U.AgRow> = U.Hookv.rows.loc(I.Ja{}, u)
    +web : String = texts(W.View.agy.rows(rs))
    +phone : String = J.Json.show(J.Json.arr(P.Mob.agy.rows(rs)))
    +shown : String = rowfields(rs)
    +bad : U.Ui = after(U.Hookv.act(u, "hook-url", "file:///etc/passwd"))
    +sec : U.Ui = after(U.Hookv.act(u, "hook-secret", "/var/hook.secret"))
    C.Check.str("english title", Bool.show(String.contains(rowtext(rs), MSG.T.hook.title(I.En{}))), "True")
    C.Check.str("english address", Bool.show(String.contains(rowtext(rs), MSG.T.hook.url(I.En{}))), "True")
    C.Check.str("japanese title", Bool.show(String.contains(rowtext(ja), MSG.T.hook.title(I.Ja{}))), "True")
    C.Check.str("japanese secret", Bool.show(String.contains(rowtext(ja), MSG.T.hook.secret(I.Ja{}))), "True")
    C.Check.str("seeded address", Bool.show(String.contains(shown, "whurl=https://example.com/hook")), "True")
    C.Check.str("seeded path", Bool.show(String.contains(shown, "whsec=/tmp/hook.secret")), "True")
    C.Check.str("path is not masked", Bool.show(String.contains(shown, "secret=False")), "True")
    C.Check.str("web shows the address", Bool.show(String.contains(web, "https://example.com/hook")), "True")
    C.Check.str("web shows Webhook", Bool.show(String.contains(web, "Webhook")), "True")
    C.Check.str("phone shows the path", Bool.show(String.contains(phone, "/tmp/hook.secret")), "True")
    C.Check.str("phone shows Webhook", Bool.show(String.contains(phone, "Webhook")), "True")
    C.Check.str("https saved", M.Setting.get(U.Ui.state.of(after(U.Hookv.act(u, "hook-url", "https://hooks.example/in?x=1"))), WN.Wn.url.key(), ""), "https://hooks.example/in?x=1")
    C.Check.str("https clears the error", U.Ui.scratch(after(U.Hookv.act(u, "hook-url", "https://hooks.example/in?x=1")), "@hook.err"), "")
    C.Check.str("file url refused", M.Setting.get(U.Ui.state.of(bad), WN.Wn.url.key(), ""), "https://example.com/hook")
    C.Check.str("file url noted", U.Ui.scratch(bad, "@hook.err"), "bad")
    C.Check.str("bad note is the sentence", Bool.show(String.contains(rowtext(U.Hookv.rows.loc(I.En{}, bad)), MSG.T.hook.url.bad(I.En{}))), "True")
    C.Check.str("empty address turns it off", M.Setting.get(U.Ui.state.of(after(U.Hookv.act(U.Ui.scratch_set(u, "@f.whurl", ""), "hook-url", ""))), WN.Wn.url.key(), "x"), "")
    C.Check.str("secret path saved", M.Setting.get(U.Ui.state.of(sec), WN.Wn.secret.key(), ""), "/var/hook.secret")
    C.Check.str("rows never carry a secret", Bool.show(String.contains(rowtext(U.Hookv.rows(sec)) ++ rowfields(U.Hookv.rows(sec)), "s3cret-value")), "False")
    C.Check.str("window save hit", Bool.show(String.contains(window(L.Set.hook(L.Ty.none(), N.Nui.init(u, 1400, 1000), 20, 680, 20)), "hook-url:")), "True")
    C.Check.str("act revoke reaches hook_revoke", Bool.show(String.contains(sends(acted("hook-revoke", "h1")), "bots.hook_revoke")), "True")
    C.Check.str("act revoke keeps the secret", M.Setting.get(U.Ui.state.of(after(acted("hook-revoke", "h1"))), WN.Wn.secret.key(), ""), "/tmp/hook.secret")
    C.Check.str("act revoke sets nothing", Bool.show(String.contains(sends(acted("hook-revoke", "h1")), "setting.set")), "False")
    C.Check.str("act token keeps the secret", M.Setting.get(U.Ui.state.of(after(acted("hook-token", "h1"))), WN.Wn.secret.key(), ""), "/tmp/hook.secret")
    C.Check.str("act retoken keeps the secret", M.Setting.get(U.Ui.state.of(after(acted("hook-retoken", "h1"))), WN.Wn.secret.key(), ""), "/tmp/hook.secret")
    C.Check.str("act untoken keeps the secret", M.Setting.get(U.Ui.state.of(after(acted("hook-untoken", "h1"))), WN.Wn.secret.key(), ""), "/tmp/hook.secret")
    C.Check.str("act new keeps the secret", M.Setting.get(U.Ui.state.of(after(acted("hook-new", ""))), WN.Wn.secret.key(), ""), "/tmp/hook.secret")
    C.Check.str("act url saves", M.Setting.get(U.Ui.state.of(after(acted("hook-url", "https://hooks.example/a"))), WN.Wn.url.key(), ""), "https://hooks.example/a")
    C.Check.str("act secret saves", M.Setting.get(U.Ui.state.of(after(acted("hook-secret", "/var/b.secret"))), WN.Wn.secret.key(), ""), "/var/b.secret")
