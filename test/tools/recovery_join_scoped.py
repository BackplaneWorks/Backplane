# Scoped compilation of the exact changed function bodies without importing
# the entire server-planner closure. Out is uninterpreted by these functions;
# the exercised Await constructor keeps its actual production field shape.
import re,hashlib,json
from pathlib import Path
s=Path('src/core/hub.bend').read_text()
def block(name,kind='def'):
 m=re.search(r'^'+kind+' '+re.escape(name)+r'(?=\(| is)[\s\S]*?(?=^(?:def |type )|\Z)',s,re.M)
 assert m,name
 return m[0].rstrip()
parts=[block('Step','type'),block('Step.none'),block('Step.cat'),block('Rec','type'),block('Rec.add')]
test=Path('test/recovery_join_test.bend').read_text().replace('import ../src/core/hub.bend as B','').replace('B.','')
imports='\n'.join(x for x in test.splitlines() if x.startswith('import '))
body='\n'.join(x for x in test.splitlines() if not x.startswith('import '))
await_line=next(l.strip() for l in s.splitlines() if l.strip().startswith('Await{thread:'))
p=Path('test/.recovery_join_focused.bend');p.write_text(imports+'\n\ntype Out is Data:\n  '+await_line+'\n\n'+'\n\n'.join(parts)+'\n\n'+body)
Path('build/real-recovery/join-scope.json').write_text(json.dumps({'kind':'exact-body scoped compilation, not whole Hub-module check','sourceSHA256':hashlib.sha256(s.encode()).hexdigest(),'blocks':[hashlib.sha256(p.encode()).hexdigest() for p in parts]},indent=2)+'\n')
