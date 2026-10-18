import Base
import ./json.bend as J
import ./kihealth.bend as KH

# Upstream schema: kicad/cli/command_project_downgrade.cpp, entries[]
# {category, feature, detail, count}. Unknown fields/categories survive in
# the raw report returned to callers; an unknown category requires consent.
type KPaths is Data:
  KPaths{base: String, output: String, report: String}

type KReport is Data:
  KReport{lower: List<&2, String>, drop: List<&2, String>, block: List<&2, String>, kept: List<&2, String>}

def KX.hash(s: String, +h: U32) -> U32:
  match s:
    case SNil{}: h
    case SCon{Chr{c}, t}: KX.hash(t, (U32.xor(h, c) * 16777619 : U32))

def KX.name(+pro: String) -> String:
  +file = Maybe.default(&2, String, List.last(&2, String, String.split(pro, '/')), "project.kicad_pro")
  String.take(file, Nat.sub(String.length(file), 10n))

# Names in managed paths contain only ASCII letters, numbers, '-' and '_'.
def KX.slug(s: String) -> String:
  match s:
    case SNil{}: ""
    case SCon{Chr{+c}, t}:
      SCon{Bool.pick(Char, Bool.or(Bool.or(Bool.and(U32.is_ge(c, 48), U32.is_le(c, 57)), Bool.and(U32.is_ge(c, 65), U32.is_le(c, 90))), Bool.or(Bool.and(U32.is_ge(c, 97), U32.is_le(c, 122)), Bool.or(U32.is_eq(c, 45), U32.is_eq(c, 95)))), Chr{c}, '-'), KX.slug(t)}

def KX.paths.base(+base: String) -> KPaths:
  KPaths{base, base ++ "/kicad10/", base ++ "/report.json"}

def KX.paths(+home: String, +pro: String) -> KPaths:
  KX.paths.base(home ++ "/exports/" ++ KX.slug(KX.name(pro)) ++ "-" ++ U32.show(KX.hash(pro, 2166136261)) ++ "-" ++ U32.show(KX.hash(pro, 131)))

def KX.base(p: KPaths) -> String:
  match p:
    case KPaths{b, _, _}: b

def KX.output(p: KPaths) -> String:
  match p:
    case KPaths{_, o, _}: o

def KX.report_path(p: KPaths) -> String:
  match p:
    case KPaths{_, _, r}: r

def KX.item(+e: J.Json) -> String:
  J.Json.get_str(e, "feature", "Unknown feature") ++ ": " ++ J.Json.get_str(e, "detail", "") ++ " (" ++ J.Json.as_raw(J.Json.get(e, "count"), "1") ++ ")"

def KX.entry(+cat: String, text: String, l: List<&2, String>, d: List<&2, String>, b: List<&2, String>, k: List<&2, String>) -> KReport:
  match cat:
    case "lower": KReport{text <> l, d, b, k}
    case "drop": KReport{l, text <> d, b, k}
    case "block": KReport{l, d, text <> b, k}
    case "keep": KReport{l, d, b, text <> k}
    case "kept": KReport{l, d, b, text <> k}
    case _: KReport{l, d, ("Unknown report category " ++ cat ++ ": " ++ text) <> b, k}

def KX.entries(es: List<&2, J.Json>, +r: KReport) -> KReport:
  match es r:
    case Nil{} _: r
    case +e <> t KReport{l, d, b, k}:
      KX.entries(t, KX.entry(J.Json.get_str(e, "category", "unknown"), KX.item(e), l, d, b, k))

def KX.report(json: J.Json) -> KReport:
  KX.entries(List.reverse(&2, J.Json, J.Json.elems(J.Json.get_obj(json, "entries"))), KReport{Nil{}, Nil{}, Nil{}, Nil{}})

def KX.valid_entries(j: J.Json) -> Bool:
  match j:
    case J.Arr{_}: True{}
    case _: False{}

def KX.valid(+j: J.Json) -> Bool:
  KX.valid_entries(J.Json.get_obj(j, "entries"))

def KX.block(r: KReport) -> Bool:
  match r:
    case KReport{_, _, b, _}: Bool.not(List.is_empty(&2, String, b))

def KX.empty(r: KReport) -> Bool:
  match r:
    case KReport{l, d, b, k}: Bool.and(Bool.and(List.is_empty(&2, String, l), List.is_empty(&2, String, d)), Bool.and(List.is_empty(&2, String, b), List.is_empty(&2, String, k)))

def KX.flags.block(blocked: Bool) -> List<&2, String>:
  match blocked:
    case True{}: ["--force", "--drop-approximations"]
    case False{}: ["--force"]

def KX.flags(r: KReport, anyway: Bool) -> List<&2, String>:
  match anyway:
    case False{}: Nil{}
    case True{}: KX.flags.block(KX.block(r))

# Lexical guard. The IO boundary additionally checks exact home/exports
# ancestry with physical paths and refuses symlinks at every managed level.
def KX.safe(+base: String) -> Bool:
  Bool.and(String.starts_with(base, "/"), Bool.and(String.contains(base, "/exports/"), Bool.and(Bool.not(String.contains(base, "/../")), Bool.and(Bool.not(String.contains(base, "/./")), Bool.and(Bool.not(String.contains(base, "//")), Bool.and(Bool.not(String.ends_with(base, "/")), Bool.not(String.ends_with(base, "/.."))))))))

def KX.may_clear(+base: String, marker_present: Bool) -> Bool:
  match marker_present:
    case False{}: False{}
    case True{}: KX.safe(base)

# The deletion boundary knows the home as well as the candidate base.
def KX.within(+home: String, +base: String) -> Bool:
  +prefix = home ++ "/exports/"
  +leaf = String.drop(base, String.length(prefix))
  Bool.and(String.starts_with(base, prefix), Bool.and(Bool.not(String.is_empty(leaf)), String.eq(leaf, KX.slug(leaf))))

def KX.clear.allowed(inside: Bool, base: String, marker: Bool) -> Bool:
  match inside:
    case False{}: False{}
    case True{}: KX.may_clear(base, marker)

def KX.may_clear_in(+home: String, +base: String, marker: Bool) -> Bool:
  KX.clear.allowed(KX.within(home, base), base, marker)

def KX.group(label: String, xs: List<&2, String>) -> List<&2, String>:
  match xs:
    case Nil{}: Nil{}
    case h <> t: label <> h <> t

def KX.lines(r: KReport) -> List<&2, String>:
  match r:
    case KReport{l, d, b, k}: List.concat(&2, String, [KX.group("Changed for KiCad 10", l), KX.group("Removed", d), KX.group("Can't be kept", b), KX.group("Kept beside the copy", k)])

# Consent follows the relevant report entries, ignoring future metadata.
def KX.strings(xs: List<&2, String>) -> List<&2, J.Json>:
  match xs:
    case Nil{}: Nil{}
    case x <> t: J.Str{x} <> KX.strings(t)

def KX.signature(report: J.Json) -> String:
  J.Json.show(J.Json.arr(KX.strings(KX.lines(KX.report(report)))))

def KX.success(path: String) -> String:
  "Exported a KiCad 10 copy to " ++ path ++ ". Changes made there don't come back into this project."
