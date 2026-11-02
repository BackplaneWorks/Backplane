import Base

# Poppler page cache. Read the bytes for the identity: an editor can replace
# a PDF several times in one second, including with one of the same size.
# A failed renderer never publishes its partial PNG as a cache entry.
# cksum is already used by the hub and is present on Linux and macOS.
def Pdf.sh() -> String:
  String.join([
    "command -v pdftoppm >/dev/null 2>&1 || { echo nopoppler; exit 3; }",
    "[ -f \"$1\" ] || { echo \"no such file\"; exit 1; }",
    "s=$(cksum < \"$1\") || { echo \"could not read PDF\"; exit 1; }",
    "k=$(printf %s \"$1\" | cksum | cut -d' ' -f1)",
    "mkdir -p \"$3\" || { echo \"could not create PDF cache\"; exit 1; }",
    "o=\"$3/v2-$k-${s%% *}-${s#* }-$2\"",
    "if [ ! -s \"$o.png\" ]; then",
    "  t=$(mktemp -d \"$3/.pdf.XXXXXX\") || { echo \"could not create PDF render directory\"; exit 1; }",
    "  trap 'rm -rf \"$t\"' 0",
    "  trap 'exit 1' 1 2 15",
    "  pdftoppm -png -r 110 -f \"$2\" -l \"$2\" -singlefile \"$1\" \"$t/page\" >/dev/null 2>&1 && [ -s \"$t/page.png\" ] || { echo \"could not draw page $2\"; exit 1; }",
    "  mv -f \"$t/page.png\" \"$o.png\" || { echo \"could not save PDF page $2\"; exit 1; }",
    "fi",
    "n=$(pdfinfo \"$1\" 2>/dev/null | awk '/^Pages:/{print $2}')",
    "printf '%s\\t%s' \"$o.png\" \"${n:-0}\""
  ], "\n")
