import Base

# Pure localization. Catalog text is compiled by scripts/i18n-gen.py.
type Loc is Data:
  En{}
  Ja{}
  Sux{}

# Base owns Zero; prefix every category consistently.
type Plural is Data:
  PZero{}
  POne{}
  PTwo{}
  PFew{}
  PMany{}
  POther{}

type Arg is Data:
  Arg{name: String, s: String, n: Nat}

def I18n.tag(l: Loc) -> String:
  match l:
    case En{}: "en"
    case Ja{}: "ja"
    case Sux{}: "sux-Xsux"

# Only two characters can name a supported language. Reading at most the
# third character avoids walking an arbitrarily long environment string.
def I18n.end(+c: Char) -> Bool:
  Bool.or(Char.is_eq(c, '-'), Bool.or(Char.is_eq(c, '_'), Bool.or(Char.is_eq(c, '.'), Char.is_eq(c, '@'))))

def I18n.language.end(s: String) -> Bool:
  match s:
    case SNil{}: True{}
    case SCon{c, _}: I18n.end(c)

def I18n.language.ja(a: Bool, b: Bool, end: Bool) -> Loc:
  match a b end:
    case True{} True{} True{}: Ja{}
    case _ _ _: En{}

def I18n.language.long(a: Char, b: Char, c: Char, +tail: String) -> Loc:
  match a b:
    case 's' 'u':
      match c tail:
        case 'x' SNil{}: Sux{}
        case 'x' SCon{'-', _}: Sux{}
        case 'x' SCon{'_', _}: Sux{}
        case 'x' SCon{'.', _}: Sux{}
        case 'x' SCon{'@', _}: Sux{}
        case _ _: En{}
    case 'j' 'a':
      match c:
        case '-': Ja{}
        case '_': Ja{}
        case '.': Ja{}
        case '@': Ja{}
        case _: En{}
    case _ _: En{}

def I18n.of_tag(s: String) -> Loc:
  match s:
    case SCon{a, SCon{b, SCon{c, t}}}:
      I18n.language.long(Char.to_lower(a), Char.to_lower(b), Char.to_lower(c), t)
    case SCon{a, SCon{b, t}}:
      I18n.language.ja(Char.is_eq(Char.to_lower(a), 'j'), Char.is_eq(Char.to_lower(b), 'a'), I18n.language.end(t))
    case _: En{}

def I18n.resolve.k(en: Bool, ja: Bool, sux: Bool, os: String) -> Loc:
  match en ja sux:
    case True{} _ _: En{}
    case False{} True{} _: Ja{}
    case False{} False{} True{}: Sux{}
    case False{} False{} False{}: I18n.of_tag(os)

def I18n.resolve(+setting: String, os: String) -> Loc:
  I18n.resolve.k(String.eq(setting, "en"), String.eq(setting, "ja"), Bool.or(String.eq(setting, "sux"), String.eq(setting, "sux-Xsux")), os)

def I18n.os.messages(empty: Bool, messages: String, lang: String) -> String:
  match empty:
    case True{}: lang
    case False{}: messages

def I18n.os.all(empty: Bool, all: String, +messages: String, lang: String) -> String:
  match empty:
    case True{}: I18n.os.messages(String.is_empty(messages), messages, lang)
    case False{}: all

def I18n.os(+lc_all: String, lc_messages: String, lang: String) -> String:
  I18n.os.all(String.is_empty(lc_all), lc_all, lc_messages, lang)

def I18n.plural.en(n: Nat) -> Plural:
  match n:
    case 0n: POther{}
    case 1n+k:
      match k:
        case 0n: POne{}
        case 1n+_: POther{}

def I18n.plural(l: Loc, n: Nat) -> Plural:
  match l:
    case En{}: I18n.plural.en(n)
    case Ja{}: POther{}
    case Sux{}: POther{}

# Walk decimal digits from right to left, prepending groups. No Nat.read,
# no per-unit Nat recursion and no growing-string comparisons.
def I18n.num.go(s: String, k: Nat, acc: String) -> String:
  match s k:
    case SNil{} _: acc
    case SCon{h, t} 0n: I18n.num.go(t, 1n, SCon{h, acc})
    case SCon{h, t} 1n+0n: I18n.num.go(t, 2n, SCon{h, acc})
    case SCon{h, t} 1n+1n+0n: I18n.num.go(t, 3n, SCon{h, acc})
    case SCon{h, t} 1n+1n+1n+_: I18n.num.go(t, 1n, SCon{h, SCon{',', acc}})

def I18n.num(l: Loc, n: Nat) -> String:
  I18n.num.go(String.reverse(Nat.show(n)), 0n, "")

def I18n.pct(l: Loc, n: Nat) -> String:
  I18n.num(l, n) ++ "%"

# Binary units; truncate the fractional tenth. Precision is determined by
# the unrounded value: one decimal below ten units, integer otherwise.
def I18n.bytes.unit.k(small: Bool, +n: Nat, +unit: Nat, name: String) -> String:
  match small:
    case True{}:
      Nat.show(Nat.div(n, unit)) ++ "." ++ Nat.show(Nat.div(Nat.mul(Nat.mod(n, unit), 10n), unit)) ++ " " ++ name
    case False{}:
      Nat.show(Nat.div(n, unit)) ++ " " ++ name

def I18n.bytes.unit(+n: Nat, +unit: Nat, name: String) -> String:
  I18n.bytes.unit.k(Nat.is_lt(n, Nat.mul(unit, 10n)), n, unit, name)

def I18n.bytes.k(b: Bool, kb: Bool, mb: Bool, +n: Nat) -> String:
  match b kb mb:
    case True{} _ _: Nat.show(n) ++ " B"
    case False{} True{} _: I18n.bytes.unit(n, 1024n, "KB")
    case False{} False{} True{}: I18n.bytes.unit(n, 1048576n, "MB")
    case False{} False{} False{}: I18n.bytes.unit(n, 1073741824n, "GB")

def I18n.bytes(l: Loc, +n: Nat) -> String:
  I18n.bytes.k(Nat.is_lt(n, 1024n), Nat.is_lt(n, 1048576n), Nat.is_lt(n, 1073741824n), n)

# Keep the English expressions identical to model.bend's Time helpers.
def I18n.ago.en(m: Bool, h: Bool, d: Bool, +s: Nat) -> String:
  match m h d:
    case True{} _ _: "now"
    case False{} True{} _: Nat.show(Nat.div(s, 60n)) ++ "m"
    case False{} False{} True{}: Nat.show(Nat.div(s, 3600n)) ++ "h"
    case False{} False{} False{}: Nat.show(Nat.div(s, 86400n)) ++ "d"

def I18n.ago.ja(m: Bool, h: Bool, d: Bool, +s: Nat) -> String:
  match m h d:
    case True{} _ _: "今"
    case False{} True{} _: Nat.show(Nat.div(s, 60n)) ++ "分前"
    case False{} False{} True{}: Nat.show(Nat.div(s, 3600n)) ++ "時間前"
    case False{} False{} False{}: Nat.show(Nat.div(s, 86400n)) ++ "日前"

def I18n.ago(l: Loc, +now: Nat, +at: Nat) -> String:
  match l:
    case En{}:
      +s = Nat.sub(now, at)
      I18n.ago.en(Nat.is_lt(s, 60n), Nat.is_lt(s, 3600n), Nat.is_lt(s, 86400n), s)
    case Ja{}:
      +s = Nat.sub(now, at)
      I18n.ago.ja(Nat.is_lt(s, 60n), Nat.is_lt(s, 3600n), Nat.is_lt(s, 86400n), s)
    case Sux{}:
      +s = Nat.sub(now, at)
      I18n.ago.en(Nat.is_lt(s, 60n), Nat.is_lt(s, 3600n), Nat.is_lt(s, 86400n), s)

def I18n.ago.long.k(l: Loc, recent: Bool, now: Nat, at: Nat) -> String:
  match l recent:
    case En{} True{}: "just now"
    case En{} False{}: I18n.ago(En{}, now, at) ++ " ago"
    case Ja{} True{}: "たった今"
    case Ja{} False{}: I18n.ago(Ja{}, now, at)
    case Sux{} True{}: "just now"
    case Sux{} False{}: I18n.ago(En{}, now, at) ++ " ago"

def I18n.ago.long(l: Loc, +now: Nat, +at: Nat) -> String:
  I18n.ago.long.k(l, Nat.is_lt(Nat.sub(now, at), 60n), now, at)

def I18n.took.en(m: Bool, h: Bool, +s: Nat) -> String:
  match m h:
    case True{} _: Nat.show(s) ++ "s"
    case False{} True{}: Nat.show(Nat.div(s, 60n)) ++ "m " ++ Nat.show(Nat.mod(s, 60n)) ++ "s"
    case False{} False{}: Nat.show(Nat.div(s, 3600n)) ++ "h " ++ Nat.show(Nat.div(Nat.mod(s, 3600n), 60n)) ++ "m"

def I18n.took.ja(m: Bool, h: Bool, +s: Nat) -> String:
  match m h:
    case True{} _: Nat.show(s) ++ "秒"
    case False{} True{}: Nat.show(Nat.div(s, 60n)) ++ "分" ++ Nat.show(Nat.mod(s, 60n)) ++ "秒"
    case False{} False{}: Nat.show(Nat.div(s, 3600n)) ++ "時間" ++ Nat.show(Nat.div(Nat.mod(s, 3600n), 60n)) ++ "分"

def I18n.took(l: Loc, +s: Nat) -> String:
  match l:
    case En{}: I18n.took.en(Nat.is_lt(s, 60n), Nat.is_lt(s, 3600n), s)
    case Ja{}: I18n.took.ja(Nat.is_lt(s, 60n), Nat.is_lt(s, 3600n), s)
    case Sux{}: I18n.took.en(Nat.is_lt(s, 60n), Nat.is_lt(s, 3600n), s)

def I18n.dur.pair(l: Loc, +a: Nat, ua: String, +b: Nat, ub: String) -> String:
  match l:
    case En{}: Nat.show(a) ++ ua ++ Bool.pick(String, Nat.is_eq(b, 0n), "", " " ++ Nat.show(b) ++ ub)
    case Ja{}: Nat.show(a) ++ ua ++ Bool.pick(String, Nat.is_eq(b, 0n), "", Nat.show(b) ++ ub)
    case Sux{}: Nat.show(a) ++ ua ++ Bool.pick(String, Nat.is_eq(b, 0n), "", " " ++ Nat.show(b) ++ ub)

def I18n.dur.k(l: Loc, sec: Bool, hour: Bool, day: Bool, +d: Nat) -> String:
  match l sec hour day:
    case En{} True{} _ _: "under a minute"
    case En{} False{} True{} _: Nat.show(Nat.div(d, 60n)) ++ "m"
    case En{} False{} False{} True{}: I18n.dur.pair(En{}, Nat.div(d, 3600n), "h", Nat.div(Nat.mod(d, 3600n), 60n), "m")
    case En{} False{} False{} False{}: I18n.dur.pair(En{}, Nat.div(d, 86400n), "d", Nat.div(Nat.mod(d, 86400n), 3600n), "h")
    case Ja{} True{} _ _: "1分未満"
    case Ja{} False{} True{} _: Nat.show(Nat.div(d, 60n)) ++ "分"
    case Ja{} False{} False{} True{}: I18n.dur.pair(Ja{}, Nat.div(d, 3600n), "時間", Nat.div(Nat.mod(d, 3600n), 60n), "分")
    case Ja{} False{} False{} False{}: I18n.dur.pair(Ja{}, Nat.div(d, 86400n), "日", Nat.div(Nat.mod(d, 86400n), 3600n), "時間")
    case Sux{} True{} _ _: "under a minute"
    case Sux{} False{} True{} _: Nat.show(Nat.div(d, 60n)) ++ "m"
    case Sux{} False{} False{} True{}: I18n.dur.pair(En{}, Nat.div(d, 3600n), "h", Nat.div(Nat.mod(d, 3600n), 60n), "m")
    case Sux{} False{} False{} False{}: I18n.dur.pair(En{}, Nat.div(d, 86400n), "d", Nat.div(Nat.mod(d, 86400n), 3600n), "h")

def I18n.dur(l: Loc, +d: Nat) -> String:
  I18n.dur.k(l, Nat.is_lt(d, 60n), Nat.is_lt(d, 3600n), Nat.is_lt(d, 86400n), d)

def I18n.until.ja(soon: Bool, d: Nat) -> String:
  match soon:
    case True{}: "まもなく"
    case False{}: I18n.dur(Ja{}, d) ++ "後"

def I18n.until(l: Loc, +d: Nat) -> String:
  match l:
    case En{}: "in " ++ I18n.dur(En{}, d)
    case Ja{}: I18n.until.ja(Nat.is_lt(d, 60n), d)
    case Sux{}: "in " ++ I18n.dur(En{}, d)

def I18n.due(l: Loc) -> String:
  match l:
    case En{}: "due now"
    case Ja{}: "実行時刻です"
    case Sux{}: "due now"

def I18n.pad.k(small: Bool, n: Nat) -> String:
  match small:
    case True{}: "0" ++ Nat.show(n)
    case False{}: Nat.show(n)

def I18n.pad(+n: Nat) -> String:
  I18n.pad.k(Nat.is_lt(n, 10n), n)

def I18n.clock(l: Loc, h: Nat, m: Nat) -> String:
  I18n.pad(h) ++ ":" ++ I18n.pad(m)

def I18n.month(m: Nat) -> String:
  match m:
    case 1n: "Jan"
    case 2n: "Feb"
    case 3n: "Mar"
    case 4n: "Apr"
    case 5n: "May"
    case 6n: "Jun"
    case 7n: "Jul"
    case 8n: "Aug"
    case 9n: "Sep"
    case 10n: "Oct"
    case 11n: "Nov"
    case _: "Dec"

def I18n.date(l: Loc, y: Nat, mo: Nat, d: Nat, this_year: Bool) -> String:
  match l this_year:
    case En{} True{}: I18n.month(mo) ++ " " ++ Nat.show(d)
    case En{} False{}: I18n.month(mo) ++ " " ++ Nat.show(d) ++ ", " ++ Nat.show(y)
    case Ja{} True{}: Nat.show(mo) ++ "月" ++ Nat.show(d) ++ "日"
    case Ja{} False{}: Nat.show(y) ++ "年" ++ Nat.show(mo) ++ "月" ++ Nat.show(d) ++ "日"
    case Sux{} True{}: I18n.month(mo) ++ " " ++ Nat.show(d)
    case Sux{} False{}: I18n.month(mo) ++ " " ++ Nat.show(d) ++ ", " ++ Nat.show(y)

def I18n.weekday.en(w: Nat) -> String:
  match w:
    case 0n: "Sun"
    case 1n: "Mon"
    case 2n: "Tue"
    case 3n: "Wed"
    case 4n: "Thu"
    case 5n: "Fri"
    case _: "Sat"

def I18n.weekday.ja(w: Nat) -> String:
  match w:
    case 0n: "日"
    case 1n: "月"
    case 2n: "火"
    case 3n: "水"
    case 4n: "木"
    case 5n: "金"
    case _: "土"

def I18n.weekday(l: Loc, w: Nat) -> String:
  match l:
    case En{}: I18n.weekday.en(w)
    case Ja{}: I18n.weekday.ja(w)
    case Sux{}: I18n.weekday.en(w)

# Hinnant's civil_from_days arithmetic, copied from Cron.civil*. An
# internal type keeps this module independent of cron.bend and its IO users.
type I18nYmd is Data:
  I18nYmd{y: Nat, mo: Nat, d: Nat}

def I18n.civil.k(early: Bool, +y: Nat, +mp: Nat, d: Nat) -> I18nYmd:
  match early:
    case True{}: I18nYmd{y, Nat.add(mp, 3n), d}
    case False{}: I18nYmd{1n+y, Nat.sub(mp, 9n), d}

def I18n.civil.mp(y: Nat, +mp: Nat, +doy: Nat) -> I18nYmd:
  I18n.civil.k(Nat.is_lt(mp, 10n), y, mp, Nat.add(Nat.sub(doy, Nat.div(Nat.add(Nat.mul(153n, mp), 2n), 5n)), 1n))

def I18n.civil.doy(+era: Nat, +yoe: Nat, +doy: Nat) -> I18nYmd:
  I18n.civil.mp(Nat.add(yoe, Nat.mul(era, 400n)), Nat.div(Nat.add(Nat.mul(5n, doy), 2n), 153n), doy)

def I18n.civil.yoe(+era: Nat, +doe: Nat, +yoe: Nat) -> I18nYmd:
  I18n.civil.doy(era, yoe, Nat.sub(doe, Nat.sub(Nat.add(Nat.mul(365n, yoe), Nat.div(yoe, 4n)), Nat.div(yoe, 100n))))

def I18n.civil.doe(+era: Nat, +doe: Nat) -> I18nYmd:
  I18n.civil.yoe(era, doe, Nat.div(Nat.sub(Nat.add(doe, Nat.div(doe, 36524n)), Nat.add(Nat.div(doe, 1460n), Nat.div(doe, 146096n))), 365n))

def I18n.civil.z(+z: Nat, +era: Nat) -> I18nYmd:
  I18n.civil.doe(era, Nat.sub(z, Nat.mul(era, 146097n)))

def I18n.civil.at(+z: Nat) -> I18nYmd:
  I18n.civil.z(z, Nat.div(z, 146097n))

def I18n.civil(days: Nat) -> I18nYmd:
  I18n.civil.at(Nat.add(days, 719468n))

def I18n.day.ymd(l: Loc, date: I18nYmd, today: I18nYmd) -> String:
  match date today:
    case I18nYmd{+y, mo, d} I18nYmd{ny, _, _}:
      I18n.date(l, y, mo, d, Nat.is_eq(y, ny))

def I18n.day(l: Loc, secs: Nat, +off: Nat, now: Nat) -> String:
  I18n.day.ymd(l, I18n.civil(Nat.div(Nat.add(secs, off), 86400n)), I18n.civil(Nat.div(Nat.add(now, off), 86400n)))

def I18n.pick(p: Plural, zero: String, one: String, two: String, few: String, many: String, other: String) -> String:
  match p:
    case PZero{}: zero
    case POne{}: one
    case PTwo{}: two
    case PFew{}: few
    case PMany{}: many
    case POther{}: other

def I18n.exact(n: Nat, k: Nat, hit: String, miss: String) -> String:
  Bool.pick(String, Nat.is_eq(n, k), hit, miss)

def I18n.sel(s: String, k: String, hit: String, miss: String) -> String:
  Bool.pick(String, String.eq(s, k), hit, miss)
