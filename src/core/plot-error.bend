import Base
import ./i18n.bend as I
import ./msgs.bend as MSG

# The plot child reports stable identifiers; each client supplies its language.
def PlotError.text(+lc: I.Loc, +text: String) -> String:
  Bool.pick(String, String.starts_with(text, "@bp.file-missing:"), MSG.T.viewer.file.missing(lc, String.drop(text, 17n)),
    Bool.pick(String, String.eq(text, "@bp.file-unread"), MSG.T.viewer.file.unread(lc), text))
