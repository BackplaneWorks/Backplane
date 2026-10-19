#!/usr/bin/env python3
"""Compile catalog metadata through the real HTML backend at several thousand keys."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('catalog_stack_gen', root / 'scripts/i18n-gen.py')
gen = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = gen
spec.loader.exec_module(gen)

keys = [f'fixture.k{i:04d}' for i in range(4097)]
missing = keys[::2]
names, lines = gen.metadata_lists({'T.keys': keys, 'T.catalog.missing.ja': missing})
gen.collision_check(names)
(root / 'build').mkdir(exist_ok=True)
with tempfile.TemporaryDirectory(prefix='i18n-catalog-stack-', dir=root / 'build') as tmp:
    folder = Path(tmp)
    (folder / 'catalog.bend').write_text('import Base\n' + '\n'.join(lines) + '''
def exported_keys() -> List<&2, String>:
  T.keys()
def exported_missing() -> List<&2, String>:
  T.catalog.missing.ja()
''')
    (folder / 'catalog.js').write_text('''import C from "./catalog.bend";
globalThis.catalogLists = {keys: C.exported_keys, missing: C.exported_missing};
''')
    (folder / 'index.html').write_text('<script type="module" src="./catalog.js"></script>\n')
    built = subprocess.run(['bend', str(folder / 'index.html'), '-o', str(folder / 'out')],
                           capture_output=True, text=True)
    (root / 'build/i18n-catalog-stack.log').write_text(built.stdout + built.stderr)
    if built.returncode:
        raise SystemExit((built.stdout + built.stderr)[-1800:])
    (folder / 'probe.js').write_text('''import {readFileSync, readdirSync} from "node:fs";
for (const f of readdirSync(process.argv[2]).filter(f => f.endsWith(".js")))
  (0, eval)(readFileSync(process.argv[2] + "/" + f, "utf8"));
function list(x) {
  const result = [];
  while (x?.$ === "Con") {
    if (result.length > 10000) throw new Error("cyclic catalog metadata");
    result.push(x.head); x = x.tail;
  }
  if (x?.$ !== "Nil") throw new Error("invalid metadata list");
  return result;
}
console.log(JSON.stringify({keys: list(catalogLists.keys()), missing: list(catalogLists.missing())}));
''')
    got = subprocess.run(['bun', str(folder / 'probe.js'), str(folder / 'out')],
                         check=True, capture_output=True, text=True)
    assert json.loads(got.stdout) == {'keys': keys, 'missing': missing}, 'catalog metadata lost order or entries'
print('ok catalog metadata HTML/JS: 4097 keys and 2049 missing, exact order and content')
