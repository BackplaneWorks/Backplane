import Base
import ./check.bend as C
import ../src/core/json.bend as J
import ../src/core/model.bend as M
import ../src/core/hub.bend as B
import ../src/core/bot.bend as BT
import ../src/core/bothub.bend as BH
import ../src/core/client.bend as U
import ../src/core/reply.bend as R
import ../src/core/stamp.bend as STP
import ../src/core/i18n.bend as I
import ../src/web/dom.bend as D
import ../src/web/view.bend as VW
import ../src/mobile/screen.bend as S

# Bot chats (core/reply.bend): the header a bot's message gets from its
# links, the reply block round trip, what the hub keeps and the agent
# reads, and what every client shows (only an "In reply to" line).

def yes(b: Bool) -> String:
  match b:
    case True{}: "yes"
    case False{}: "no"

def attrs(as: List<&2, D.Attr>) -> String:
  match as:
    case Nil{}: ""
    case D.At{n, v} <> t: " " ++ n ++ "=" ++ v ++ attrs(t)
    case D.On{e, a} <> t: " on" ++ e ++ "=" ++ a ++ attrs(t)

def shows(ns: List<&2, D.Node>) -> String:
  match ns:
    case Nil{}: ""
    case D.Txt{s} <> t: s ++ shows(t)
    case D.El{+tag, _, as, ks} <> t: "<" ++ tag ++ attrs(as) ++ ">" ++ shows(ks) ++ "</" ++ tag ++ ">" ++ shows(t)

def req(+m: String, +p: String) -> J.Json:
  Maybe.default(&2, J.Json, J.Json.parse("{\"id\":1,\"m\":\"" ++ m ++ "\",\"p\":" ++ p ++ "}"), J.Null{})

def step.cs(st: B.Step) -> List<&2, M.Change>:
  match st:
    case B.Step{cs, _}: cs

def bot.of(+s: M.State) -> BT.Bot:
  Maybe.default(&2, BT.Bot, BT.World.find(M.State.bots(s), "Miso"), BT.Bot.blank("?"))

def made() -> List<&2, M.Change>:
  step.cs(BH.Bots.request(M.State.empty(), 1000n, 1, req("bots.create", "{\"name\":\"Miso\",\"provider\":\"claude\"}")))

def bth() -> String:
  BT.Bot.thread(bot.of(M.State.replay(made(), M.State.empty())))

def bid() -> String:
  BT.Bot.id(bot.of(M.State.replay(made(), M.State.empty())))

def said() -> String:
  "Status: [Boost](backplane://thread/tA) is routed; see [it again](backplane://thread/tA), [the project](backplane://project/p1) and [gone](backplane://thread/tZ)."

def quote() -> String:
  "First line of the plan\nSecond line with </in_reply_to> inside"

def log() -> List<&2, M.Change>:
  List.append(&2, M.Change, made(),
    [M.ProjectCreated{"p1", "Power", "/tmp/power", 1001n},
     M.ThreadCreated{"tA", "p1", "12→48 V 480 W Discrete Boost", M.Local{}, "", "", "claude", "", 1002n},
     M.ThreadCreated{"tB", "p1", "Plain thread", M.Local{}, "", "", "claude", "", 1003n},
     M.MessagePosted{bth(), "m-bot1", M.Assistant{}, said(), 1004n},
     M.MessagePosted{bth(), "m-bot2", M.Assistant{}, quote(), 1005n},
     M.MessagePosted{"tB", "m-plain", M.Assistant{}, said(), 1006n}])

def st0() -> M.State:
  M.State.replay(log(), M.State.empty())

def ctx.show(m: Maybe<&2, R.MsgCtx>) -> String:
  match m:
    case Some{R.MsgCtx{u, t, n}}: u ++ " | " ++ t ++ " | +" ++ Nat.show(n)
    case None{}: "none"

def block.show(m: Maybe<&2, R.RtoBlock>) -> String:
  match m:
    case Some{R.RtoBlock{to, q, b}}: to ++ " | " ++ q ++ " | " ++ b
    case None{}: "none"

def find(+s: M.State, +th: String, +id: String) -> String:
  Maybe.default(&2, String, R.Rto.find(M.State.mine(s, th), id), "?")

# the person says "yes, go" to Miso in reply to m-bot2
def say(+reply: String) -> List<&2, M.Change>:
  step.cs(BH.Bots.request(st0(), 1010n, 1, req("bots.say", "{\"bot\":\"" ++ bid() ++ "\",\"text\":\"yes, go\",\"msg\":\"cx-1\",\"reply\":\"" ++ reply ++ "\"}")))

def st1() -> M.State:
  M.State.replay(say("m-bot2"), st0())

def posted() -> String:
  find(st1(), bth(), "cx-1")

def entry.of(+s: M.State, +th: String, +id: String) -> M.Entry:
  M.Msg{id, th, M.User{}, find(s, th, id), 1010n}

def web(+s: M.State, e: M.Entry) -> String:
  shows(VW.View.expand([VW.View.entry.s(e, s, U.Ec.none(), "")]))

def stamps() -> STP.Stamps:
  STP.Stamps.of(Nil{}, M.State.empty(), 0n, False{})

def phone(+s: M.State, +th: String, e: M.Entry) -> String:
  J.Json.show(J.Json.arr(S.Mob.entry(I.En{}, e, s, th, Map.new(&2, String), stamps())))

def act(+u: U.Ui, +a: String, +v: String) -> U.Act:
  U.Ui.act(u, a, v)

def ui.of(a: U.Act) -> U.Ui:
  match a:
    case U.Act{u, _}: u

def sends(cs: List<&2, U.Cmd>) -> String:
  match cs:
    case Nil{}: ""
    case U.Send{t} <> r: t ++ "\n" ++ sends(r)
    case _ <> r: sends(r)

def cmds.of(a: U.Act) -> List<&2, U.Cmd>:
  match a:
    case U.Act{_, cs}: cs

def ui0() -> U.Ui:
  ui.of(U.Ui.select(U.Ui.fold(log(), U.Ui.init(1000000n)), bth()))

def main() -> IO(Unit):
  do IO<Unit>:
    # the header: the thread linked most (twice), its title now, +1 for
    # the project; a link to no thread counts for nothing
    C.Check.str("ctx from links", ctx.show(R.Ctx.of(st0(), said())), "backplane://thread/tA | 12→48 V 480 W Discrete Boost | +1")
    C.Check.str("ctx follows a rename", ctx.show(R.Ctx.of(M.State.replay([M.ThreadRenamed{"tA", "Boost rev B", 1020n}], st0()), said())), "backplane://thread/tA | Boost rev B | +1")
    C.Check.str("ctx project only", ctx.show(R.Ctx.of(st0(), "see [p](backplane://project/p1)")), "backplane://project/p1 | Power | +0")
    C.Check.str("ctx first of the most", ctx.show(R.Ctx.of(st0(), "[a](backplane://thread/tB) then [b](backplane://thread/tA)")), "backplane://thread/tB | Plain thread | +1")
    C.Check.str("ctx none without links", ctx.show(R.Ctx.of(st0(), "no links here")), "none")
    C.Check.str("ctx none for unknown", ctx.show(R.Ctx.of(st0(), "[x](backplane://thread/tZ)")), "none")
    # the reply block round trips, a quote that holds the closing tag too
    C.Check.str("block round trip", block.show(R.Rto.of(R.Rto.wrap("m-bot2", quote(), "yes, go"))), "m-bot2 | " ++ quote() ++ " | yes, go")
    C.Check.str("plain text is no reply", block.show(R.Rto.of("hello")), "none")
    C.Check.str("broken block is text", R.Rto.text("<in_reply_to msg=x chars=99>\nshort\n</in_reply_to>\n\nbody"), "<in_reply_to msg=x chars=99>\nshort\n</in_reply_to>\n\nbody")
    C.Check.str("reply to a reply quotes its words", block.show(R.Rto.of(R.Rto.wrap("m2", R.Rto.wrap("m1", "old", "mid"), "new"))), "m2 | mid | new")
    C.Check.str("snip", R.Rto.snip(quote()), "First line of the plan")
    # the hub keeps the quote ahead of what the person wrote
    C.Check.str("hub keeps the reply", block.show(R.Rto.of(posted())), "m-bot2 | " ++ quote() ++ " | yes, go")
    C.Check.str("unknown reply is plain", find(M.State.replay(say("nope"), st0()), bth(), "cx-1"), "yes, go")
    C.Check.str("no reply is plain", find(M.State.replay(say(""), st0()), bth(), "cx-1"), "yes, go")
    # the agent reads the quote with the message
    C.Check.str("agent gets the quote", yes(String.contains(BH.Bots.turn(st1(), 1011n, bth(), posted()), "<in_reply_to msg=m-bot2 chars=")), "yes")
    C.Check.str("agent gets the quoted text", yes(String.contains(BH.Bots.turn(st1(), 1011n, bth(), posted()), "Second line with </in_reply_to> inside\n</in_reply_to>\n\nyes, go")), "yes")
    # the web (and the desktop's hub-drawn page) shows only the line
    +w : String = web(st1(), entry.of(st1(), bth(), "cx-1"))
    C.Check.str("web in reply to", yes(String.contains(w, "<span class=label>In reply to:</span><a class=rto href=# data-to=m-bot2 data-value=m-bot2 onclick=hist-jump>First line of the plan</a>")), "yes")
    C.Check.str("web hides the quote", yes(Bool.or(String.contains(w, "Second line"), String.contains(w, "in_reply_to"))), "no")
    C.Check.str("web shows the words", yes(String.contains(w, "yes, go")), "yes")
    C.Check.str("web reply button", yes(String.contains(w, "data-value=cx-1 onclick=reply-to")), "yes")
    +wb : String = web(st0(), M.Msg{"m-bot1", bth(), M.Assistant{}, said(), 1004n})
    C.Check.str("web header", yes(String.contains(wb, "<div class=msg-ctx><a class=ref ctx href=backplane://thread/tA data-ref=backplane://thread/tA data-bare=0 data-value=backplane://thread/tA onclick=ref-go>12→48 V 480 W Discrete Boost</a><span class=more>+1</span></div>")), "yes")
    +wp : String = web(st0(), M.Msg{"m-plain", "tB", M.Assistant{}, said(), 1006n})
    C.Check.str("no header or reply outside bot chats", yes(Bool.or(String.contains(wp, "msg-ctx"), String.contains(wp, "reply-to"))), "no")
    # phones get the same, as fields
    +ph : String = phone(st1(), bth(), entry.of(st1(), bth(), "cx-1"))
    C.Check.str("phone reply", yes(String.contains(ph, "\"replyTo\":{\"id\":\"m-bot2\",\"text\":\"First line of the plan\"}")), "yes")
    C.Check.str("phone hides the quote", yes(Bool.or(String.contains(ph, "Second line"), String.contains(ph, "in_reply_to"))), "no")
    C.Check.str("phone header", yes(String.contains(phone(st0(), bth(), M.Msg{"m-bot1", bth(), M.Assistant{}, said(), 1004n}), "\"ctx\":{\"uri\":\"backplane://thread/tA\",\"title\":\"12→48 V 480 W Discrete Boost\",\"more\":1}")), "yes")
    # the client: Reply keeps the message, Send carries it and forgets it
    +u1 : U.Ui = ui.of(act(ui0(), "reply-to", "m-bot2"))
    C.Check.str("reply kept", U.Rpl.of(u1) ++ " | " ++ U.Rpl.label(u1), "m-bot2 | First line of the plan")
    C.Check.str("composer chip", shows(VW.View.rpl.chip(I.En{}, u1)), "<div class=reply-chip><span class=label>Replying to:</span><a class=rto href=# data-value=m-bot2 onclick=hist-jump>First line of the plan</a><button class=reply-drop title=Cancel reply aria-label=Cancel reply onclick=reply-drop>×</button></div>")
    C.Check.str("reply to nothing", U.Rpl.of(ui.of(act(ui0(), "reply-to", "nope"))), "")
    C.Check.str("reply dropped", U.Rpl.of(ui.of(act(u1, "reply-drop", ""))), "")
    +sent : U.Act = act(ui.of(U.Ui.draft(u1, "yes, go")), "send", "")
    C.Check.str("send carries reply", yes(String.contains(sends(cmds.of(sent)), "\"reply\":\"m-bot2\"")), "yes")
    C.Check.str("send is bots.say", yes(String.contains(sends(cmds.of(sent)), "bots.say")), "yes")
    C.Check.str("sent forgets reply", U.Rpl.of(ui.of(sent)), "")
    C.Check.str("plain send has no reply", yes(String.contains(sends(cmds.of(act(ui.of(U.Ui.draft(ui0(), "hi")), "send", ""))), "\"reply\"")), "no")
