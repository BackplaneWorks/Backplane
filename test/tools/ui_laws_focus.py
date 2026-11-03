#!/usr/bin/env python3
"""Check the actual new law/proof blocks cheaply before the complete proof gate."""
from pathlib import Path
import re

root = Path(__file__).resolve().parents[2]
laws = (root / 'LAWS.bend').read_text()
proof = (root / 'PROOF.bend').read_text()
claims = [
    'ui_navigation_total', 'ui_view_error_preserves_all_drafts',
    'ui_state_adoption_preserves_all_drafts', 'ui_bottom_has_no_unread_pill',
    'ui_seen_boundary_has_no_unread_pill', 'json_low_surrogate_replaced',
    'json_high_surrogate_waits', 'json_pair_unit_combines',
    'json_unpaired_high_preserves_next_character',
    'json_unpaired_high_preserves_next_escape',
    'ui_stored_echo_removes_pending_preview', 'sync_credit_report_duplicate',
    'sync_credit_reports_commute',
]

def block(text, kind, name):
    found = list(re.finditer(r'^' + kind + r' ' + re.escape(name) + r'(?=[:(])', text, re.M))
    if len(found) != 1:
        raise SystemExit(f'Expected exactly one {kind} {name}, got {len(found)}')
    start = found[0].start()
    tail = text[found[0].end():]
    stop = re.search(r'^(?:law|def|type|import|@unsafe)\b', tail, re.M)
    return text[start:found[0].end() + stop.start() if stop else len(text)].rstrip() + '\n'

parts = ['import Base\n']
for path, alias in [('src/core/client.bend', 'U'), ('src/core/model.bend', 'M'),
                    ('src/core/json.bend', 'J'), ('src/core/outbox.bend', 'OB'),
                    ('test/ui_laws_proof.bend', 'UILAWS'),
                    ('test/sync_report_proof.bend', 'SYNCREPORT')]:
    parts.append(f'import ../../{path} as {alias}\n')
for name in ['bool_cmp_refl', 'word_cmp_refl', 'u32_cmp_refl', 'string_cmp_refl', 'and_false_r']:
    parts.extend([block(proof, 'law', name), block(proof, 'def', name)])
parts.extend([block(laws, 'law', 'string_eq_refl'), block(proof, 'def', 'Laws.string_eq_refl').replace('Laws.', '')])
for name in claims:
    parts.extend([block(laws, 'law', name), block(proof, 'def', 'Laws.' + name).replace('Laws.', '')])
output = root / 'build/ui-laws/focused.bend'
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text('\n'.join(parts))
print(f'Focused actual law blocks: {len(claims)} (does not replace bend PROOF.bend)')
