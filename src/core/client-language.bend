import Base
import ./i18n.bend as I

# Pure device language policy shared by client state and native entry points.

def Lang.ready() -> Bool:
  False{}

def Lang.normalize.k(en: Bool, ja: Bool) -> String:
  match en ja:
    case True{} _: "en"
    case False{} True{}: "ja"
    case False{} False{}: "auto"

def Lang.normalize(+s: String) -> String:
  Lang.normalize.k(String.eq(s, "en"), String.eq(s, "ja"))

def Lang.resolve.k(+l: I.Loc, allow: Bool) -> String:
  match l allow:
    case I.Ja{} True{}: "ja"
    case _ _: "en"

def Lang.resolve(setting: String, os: String, preview: Bool) -> String:
  Lang.resolve.k(I.I18n.resolve(setting, os), Bool.or(Lang.ready(), preview))

def native_locale(setting: String, os: String, preview: String) -> String:
  Lang.resolve(Lang.normalize(setting), os, String.eq(preview, "1"))
