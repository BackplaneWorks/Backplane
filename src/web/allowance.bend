import Base
import ../core/allowance.bend as A
import ../core/usage.bend as UZ
import ./dom.bend as D
import ../core/json.bend as J

def AllowanceWeb.chip(+snapshot: String, +provider: String, +model: String, +now: Nat, +open: Bool) -> List<&2, D.Node>:
  [D.El{"button", "allowance-chip", [D.At{"class", "allowance-chip"}, D.At{"id", "allowance-chip"}, D.At{"type", "button"},
    D.At{"aria-expanded", Bool.pick(String, open, "true", "false")}, D.At{"aria-controls", "allowance-details"},
    D.At{"data-as-of", Nat.show(now)}, D.At{"data-valid-until", Nat.show(A.Allowance.until(snapshot, provider, model))},
    D.At{"title", "Remaining provider allowance · details and reset times"}, D.On{"click", "allowance-open"}],
    [D.El{"span", "allowance-current", [D.At{"class", "allowance-current"}], [D.Txt{A.Allowance.chip(snapshot, provider, model, now)}]}, D.Dom.span("allowance-retired", "Limits stale")]}]

def AllowanceWeb.rows(rs: List<&2, UZ.URow>) -> List<&2, D.Node>:
  match rs:
    case Nil{}: Nil{}
    case +r <> rest:
      D.El{"div", "", [D.At{"class", "allowance-row"}], [D.El{"dt", "", Nil{}, [D.Txt{UZ.URow.label(r)}]},
        D.El{"dd", "", Nil{}, [D.Txt{UZ.URow.note(r)}]}]} <> AllowanceWeb.rows(rest)

def AllowanceWeb.can_read(+snapshot: String, +provider: String) -> Bool:
  String.eq(provider, "codex") && Bool.not(J.Json.get_bool(A.Allowance.parse(snapshot), "managed_account", False{}))

def AllowanceWeb.pop(open: Bool, +snapshot: String, +provider: String, +model: String, +now: Nat) -> List<&2, D.Node>:
  match open:
    case True{}:
      [D.El{"div", "allowance-shade", [D.At{"class", "shade allowance-shade"}, D.At{"aria-hidden", "true"}, D.On{"click", "allowance-close"}], Nil{}},
       D.El{"section", "allowance-details", [D.At{"class", "allowance-pop"}, D.At{"id", "allowance-details"}, D.At{"role", "region"}, D.At{"aria-label", "Remaining allowance"}],
         List.append(&2, D.Node, [D.El{"div", "", [D.At{"class", "allowance-head"}], [D.Dom.span("", "Remaining allowance"),
           D.El{"button", "allowance-close", [D.At{"id", "allowance-close"}, D.At{"type", "button"}, D.On{"click", "allowance-close"}], [D.Txt{"Close"}]}]},
          D.Dom.span("allowance-expired", "Limits stale · this presentation has expired. Last reported values follow."),
          D.El{"dl", "", Nil{}, AllowanceWeb.rows(A.Allowance.rows(snapshot, provider, model, now))},
          D.Dom.span("allowance-note", "Provider limits include usage outside Backplane. Local token totals do not measure allowance."),
          D.Dom.span("allowance-note", Bool.pick(String, J.Json.get_bool(A.Allowance.parse(snapshot), "managed_account", False{}), "Selected account metadata; Status / Refresh is available in Settings → Agents. Pools may also be shared with other profiles.", Bool.pick(String, String.eq(provider, "codex"), "Checked at most every 5 minutes. Changing models makes no provider call.", "Updates when the provider reports a limit during a turn.")))],
         Bool.pick(List<&2, D.Node>, AllowanceWeb.can_read(snapshot, provider), [D.Dom.button("", "allowance-read", "", "Read provider limits (cached for 5 minutes)", "Check limits")], Nil{}))}]
    case False{}: Nil{}
