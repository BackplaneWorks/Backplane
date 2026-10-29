"""An uploaded quote package: the manifest and its files, checked before
anything reads them.

The hub sends multipart/form-data: a `manifest` field (JSON) and one `file`
part per file, whose filename is the file's path inside the package. Every
file must be listed in the manifest with its SHA-256, and nothing unlisted is
kept. Gerber archives (.zip) are unpacked here, with the same checks, one
level deep.
"""
import hashlib
import json
import pathlib
import re
import zipfile

from . import catalog, config

SPEC = 1
COUNTRY = re.compile(r"^[A-Z]{2}$")
# ISO 3166-1 alpha-2: the countries a quote may ship to. Two letters is not
# enough: "EU" passed, and shipping could not be priced for it
ISO = frozenset("""AD AE AF AG AI AL AM AO AQ AR AS AT AU AW AX AZ BA BB BD BE BF BG BH BI BJ BL BM BN BO BQ BR BS BT BV BW
BY BZ CA CC CD CF CG CH CI CK CL CM CN CO CR CU CV CW CX CY CZ DE DJ DK DM DO DZ EC EE EG EH ER ES ET FI FJ FK FM FO FR GA
GB GD GE GF GG GH GI GL GM GN GP GQ GR GS GT GU GW GY HK HM HN HR HT HU ID IE IL IM IN IO IQ IR IS IT JE JM JO JP KE KG KH
KI KM KN KP KR KW KY KZ LA LB LC LI LK LR LS LT LU LV LY MA MC MD ME MF MG MH MK ML MM MN MO MP MQ MR MS MT MU MV MW MX MY
MZ NA NC NE NF NG NI NL NO NP NR NU NZ OM PA PE PF PG PH PK PL PM PN PR PS PT PW PY QA RE RO RS RU RW SA SB SC SD SE SG SH
SI SJ SK SL SM SN SO SR SS ST SV SX SY SZ TC TD TF TG TH TJ TK TL TM TN TO TR TT TV TW TZ UA UG UM US UY UZ VA VC VE VG VI
VN VU WF WS YE YT ZA ZM ZW""".split())
REGIONS = {"EU": "the European Union", "EZ": "the eurozone", "UN": "the United Nations", "UK": "the United Kingdom (its code is GB)"}
NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9 ._-]{0,63}$")
PATH_PART = re.compile(r"^[A-Za-z0-9][A-Za-z0-9 ._+()-]{0,127}$")
SHA = re.compile(r"^[0-9a-f]{64}$")

GERBER_RE = re.compile(r"^\.(g[a-z0-9]{1,3}|gbr|gbrjob|drl|xln|exc|txt|nc|cnc)$")


def is_gerber(name):
    return bool(GERBER_RE.match(pathlib.PurePosixPath(name).suffix.lower()))


OTHER_EXT = {".zip", ".csv", ".step", ".stp", ".dxf", ".json", ".md"}
# what each kind may carry, and what it must
FILE_ROLES = {
    **{k: ({"gerbers", "bom", "positions"}, {"gerbers"}) for k in catalog.BOARDS},
    "3dp": ({"step"}, {"step"}),
    "cnc": ({"step", "drawing"}, {"step"}),
    "sheetmetal": ({"step", "dxf"}, {"step"}),
}
MAX_PARTS = 20


class Bad(Exception):
    """The package is refused; the message is safe to show to the sender."""


def safe_path(p):
    """A relative path with plain components only (no '..', no absolute paths, no odd characters)."""
    if not isinstance(p, str) or not p or len(p) > 300 or p.startswith("/") or "\\" in p:
        raise Bad(f"bad file path: {p!r}")
    parts = p.split("/")
    for c in parts:
        if not PATH_PART.match(c) or c in (".", ".."):
            raise Bad(f"bad file path: {p!r}")
    return "/".join(parts)


def check_manifest(m):
    """The manifest's shape, its parts' options and the file list. Returns the cleaned manifest."""
    if not isinstance(m, dict) or m.get("spec") != SPEC:
        raise Bad(f"manifest spec must be {SPEC}; update Backplane")
    country = m.get("ship_to")
    if not isinstance(country, str) or not COUNTRY.match(country):
        raise Bad("ship_to must be a country's two-letter ISO code, like US, DE or GR")
    if country not in ISO:
        what = REGIONS.get(country)
        raise Bad(f"ship_to {country} is {what}, not a country: ask which country it ships to" if what
                  else f"ship_to {country} is not a country's ISO code: ask which country it ships to")
    files = m.get("files")
    if not isinstance(files, dict) or not files or len(files) > config.MAX_FILES:
        raise Bad("files must list each file's sha256")
    clean_files = {}
    for path, sha in files.items():
        if not isinstance(sha, str) or not SHA.match(sha):
            raise Bad(f"bad sha256 for {path}")
        if not is_gerber(path) and pathlib.PurePosixPath(path).suffix.lower() not in OTHER_EXT:
            raise Bad(f"file type not accepted: {path}")
        clean_files[safe_path(path)] = sha
    parts = m.get("parts")
    if not isinstance(parts, list) or not parts or len(parts) > MAX_PARTS:
        raise Bad(f"parts must list 1 to {MAX_PARTS} parts")
    out, names = [], set()
    for p in parts:
        if not isinstance(p, dict):
            raise Bad("each part must be an object")
        name, kind = p.get("name"), p.get("kind")
        if not isinstance(name, str) or not NAME.match(name) or name in names:
            raise Bad(f"bad or repeated part name: {name!r}")
        names.add(name)
        opts, feats, problems = catalog.clean(kind, p.get("options"), p.get("features"), need_features=False)
        if problems:
            raise Bad(f"{name}: " + "; ".join(problems))
        qty = p.get("qty")
        if isinstance(qty, bool) or not isinstance(qty, int) or not (1 <= qty <= 100000):
            raise Bad(f"{name}: qty must be a whole number from 1 to 100000")
        allowed, needed = FILE_ROLES[kind]
        pf = p.get("files")
        if not isinstance(pf, dict):
            raise Bad(f"{name}: files must be an object")
        roles = {}
        for role, path in pf.items():
            if role not in allowed:
                raise Bad(f"{name}: unknown file role {role!r} for {kind}")
            path = safe_path(path.rstrip("/")) if isinstance(path, str) else safe_path(path)
            if path not in clean_files and not any(f.startswith(path + "/") for f in clean_files):
                raise Bad(f"{name}: {role} {path} is not in the file list")
            roles[role] = path
        for role in needed:
            if role not in roles:
                raise Bad(f"{name}: a {kind} needs its {role}")
        assemble = p.get("assemble", 0)
        if kind not in catalog.ASSEMBLED and assemble:
            raise Bad(f"{name}: only rigid FR-4 boards (pcb) are assembled" if kind in catalog.BOARDS
                      else f"{name}: only boards are assembled")
        if isinstance(assemble, bool) or not isinstance(assemble, int) or not (0 <= assemble <= qty):
            raise Bad(f"{name}: assemble is how many of the {qty} boards to assemble")
        if assemble and assemble < catalog.MIN_ASSEMBLED:
            raise Bad(f"{name}: assembly starts at {catalog.MIN_ASSEMBLED} boards")
        if assemble and not ("bom" in roles and "positions" in roles):
            raise Bad(f"{name}: assembly needs the bom and positions files")
        # the options the person chose (the rest are defaults, free to follow the board, catalog.board_rules)
        given = sorted(k for k in p["options"]) if isinstance(p.get("options"), dict) else []
        out.append({"name": name, "kind": kind, "qty": qty, "options": opts, "features": feats,
                    "files": roles, "assemble": assemble, "given": given})
    title = m.get("title", "")
    title = title[:80] if isinstance(title, str) else ""
    return {"spec": SPEC, "ship_to": country, "title": title, "parts": out, "files": clean_files}


def _unzip(zpath, dest, budget):
    """Unpack a gerber archive next to itself: plain files only, bounded size, no paths outside dest."""
    try:
        z = zipfile.ZipFile(zpath)
    except zipfile.BadZipFile:
        raise Bad(f"{zpath.name} is not a zip archive")
    with z:
        infos = z.infolist()
        if len(infos) > config.MAX_FILES:
            raise Bad(f"{zpath.name} holds too many files")
        for i in infos:
            if i.is_dir():
                continue
            if (i.external_attr >> 16) & 0o170000 == 0o120000:
                raise Bad(f"{zpath.name}: links are not accepted")
            name = pathlib.PurePosixPath(i.filename).name
            if not name or not PATH_PART.match(name):
                raise Bad(f"{zpath.name}: bad name {i.filename!r}")
            if not is_gerber(name):
                continue
            if i.file_size > 50 * 1024 * 1024 or (i.compress_size and i.file_size / i.compress_size > 300):
                raise Bad(f"{zpath.name}: {name} unpacks too large")
            budget[0] -= i.file_size
            if budget[0] < 0:
                raise Bad("the package unpacks too large")
            target = dest / name
            with z.open(i) as src, open(target, "wb") as out:
                left = i.file_size + 1
                while True:
                    chunk = src.read(1 << 16)
                    if not chunk:
                        break
                    left -= len(chunk)
                    if left < 0:
                        raise Bad(f"{zpath.name}: {name} is larger than it says")
                    out.write(chunk)


def store_files(manifest, uploads, root):
    """Write the uploaded files under root/files, checking each against the manifest's hash.
    uploads: [(path, bytes)]. Archives named as a part's gerbers are unpacked to <archive>.d/."""
    files = root / "files"
    files.mkdir(parents=True)
    seen = set()
    total = 0
    for path, data in uploads:
        path = safe_path(path)
        want = manifest["files"].get(path)
        if want is None:
            raise Bad(f"{path} is not in the manifest")
        if path in seen:
            raise Bad(f"{path} was sent twice")
        if hashlib.sha256(data).hexdigest() != want:
            raise Bad(f"{path} does not match its sha256")
        total += len(data)
        if total > config.MAX_UPLOAD:
            raise Bad("the package is too large")
        seen.add(path)
        target = files / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    missing = set(manifest["files"]) - seen
    if missing:
        raise Bad("missing files: " + ", ".join(sorted(missing)[:5]))
    budget = [config.MAX_UNPACKED]
    for p in manifest["parts"]:
        g = p["files"].get("gerbers")
        if g and g.lower().endswith(".zip"):
            d = files / (g + ".d")
            d.mkdir(parents=True, exist_ok=True)
            _unzip(files / g, d, budget)
    (root / "manifest.json").write_text(json.dumps(manifest))
