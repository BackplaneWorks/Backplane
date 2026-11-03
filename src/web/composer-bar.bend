import Base
import ./dom.bend as D

# A content-independent layout: labels and reply banners cannot add rows.
# The renderer supplies the responsive button size through one CSS variable.
type ControlBar is Data:
  ControlBar{height: String, wrap: String}

def ComposerBar.layout(button: String) -> ControlBar:
  ControlBar{button, "nowrap"}

def ComposerBar.height(layout: ControlBar) -> String:
  match layout:
    case ControlBar{height, _}: height

def ComposerBar.wrap(layout: ControlBar) -> String:
  match layout:
    case ControlBar{_, wrap}: wrap

def ComposerBar.style(+layout: ControlBar) -> String:
  "display:flex;flex-wrap:" ++ ComposerBar.wrap(layout) ++ ";height:" ++ ComposerBar.height(layout) ++ ";min-height:" ++ ComposerBar.height(layout) ++ ";max-height:" ++ ComposerBar.height(layout)

def ComposerBar.node(kids: List<&2, D.Node>) -> D.Node:
  D.El{"div", "composer-foot", [D.At{"class", "composer-foot"}, D.At{"style", ComposerBar.style(ComposerBar.layout("var(--composer-button)"))}], kids}

def ComposerBar.more(kids: List<&2, D.Node>) -> D.Node:
  D.El{"details", "composer-more", [D.At{"class", "local-menu composer-more"}], [
    D.El{"summary", "", [D.At{"aria-label", "More composer controls"}, D.At{"title", "More composer controls"}], [D.Txt{"⋯"}]},
    D.El{"div", "", [D.At{"class", "local-menu-body composer-options"}], kids}]}
