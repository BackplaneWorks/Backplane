import Base
import ./i18n.bend as I
import ./msgs.bend as MSG

def Title.shown.k(fresh: Bool, l: I.Loc, title: String) -> String:
  match fresh:
    case True{}: MSG.T.thread.new(l)
    case False{}: title

def Title.shown(l: I.Loc, +title: String) -> String:
  Title.shown.k(String.eq(title, "New thread"), l, title)
