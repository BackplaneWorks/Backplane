import Base
import ../core/adopt.bend as AD

# Pure adopt plan policy shared by production and its laws.

def Adopt.copy.entries(copy: Bool, +rec: String, +hand: String) -> List<&2, AD.AEntry>:
  match copy:
    case True{}:
      Nil{}
    case False{}:
      AD.Adopt.entries(rec, hand)
