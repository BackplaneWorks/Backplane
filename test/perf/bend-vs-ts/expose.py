"""Expose actual emitted internal functions, without host Nat marshalling.

Only replace the two final entry-point calls. The hub is never started.
Fail closed if the compiler's bootstrap or symbol naming changes.
"""
import json
import re
import sys
from pathlib import Path

source = Path(sys.argv[1]).read_text()
exports = {
    'empty': ('model', 'State.empty'),
    'apply': ('model', 'State.apply'),
    'replay': ('model', 'State.replay'),
    'sideIndex': ('client', 'Side.ix'),
    'phase': ('client', 'Side.phase.ix'),
    'phaseScan': ('client', 'Side.phase'),
    'phaseName': ('model', 'Phase.name'),
    'touch': ('thin-pace', 'Touch.change'),
    'touchAdd': ('thin-pace', 'Touch.adds'),
    'touchLevel': ('thin-pace', 'Touch.level.for'),
    'pace': ('thin-pace', 'Pace.step'),
    'page': ('thinstore', 'Ts.page'),
    'pageIds': ('thinstore', 'Page.ids'),
    'indexEmpty': ('thin', 'Index.empty'),
    'blocks': ('thin', 'Blocks.of'),
    'dbOpen': ('db', 'Db.open'),
    'dbClose': ('db', 'Db.close'),
    'mapSet': ('Base', 'Map.set'),
    'mapGet': ('Base', 'Map.get'),
    'mapValues': ('Base', 'Map.values'),
}
names = re.findall(r'function\s+([\w$]+)\(', source)
out = []
for key, (module, definition) in exports.items():
    mod = module.replace('-', '$045')
    suffix = (('$047' + mod if module not in ('thinstore', 'db') else '$' + mod) + '$058' if module != 'Base' else '$') + definition.replace('.', '$') + '$'
    found = [name for name in names if name.endswith(suffix)]
    if len(found) != 1:
        raise SystemExit(f'{key}: expected one {suffix}, got {found}')
    out.append(f'{json.dumps(key)}: {found[0]}')
bootstrap = re.compile(r'cli\(process\.argv\.slice\(1\)\);\s*io_exit\(\$main\$,\s*null\);?')
source, n = bootstrap.subn('globalThis.__bendBench = {' + ',\n'.join(out) + ', effects: $0eff};', source)
if n != 1:
    raise SystemExit('Expected exactly one hub bootstrap; refusing to import')
source += '\nexport const bench = globalThis.__bendBench;\n'
Path(sys.argv[2]).write_text(source)
