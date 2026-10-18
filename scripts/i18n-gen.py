#!/usr/bin/env python3
"""Compile the small ICU catalog subset into pure, deterministic Bend defs."""
import argparse
from dataclasses import dataclass
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parent.parent
KEY = re.compile(r'[a-z][a-z0-9]*(?:\.[a-z0-9]+)*\Z')
NAME = re.compile(r'[A-Za-z_][A-Za-z0-9_]*\Z')
KEYWORDS = set('match case def type is where if else return import as law do with use let fork for when switch true false'.split())
CATEGORIES = ('zero', 'one', 'two', 'few', 'many', 'other')
LOCALES = (('en', 'En'), ('ja', 'Ja'))


class CatalogError(ValueError):
    pass


@dataclass(frozen=True)
class Text:
    value: str


@dataclass(frozen=True)
class Value:
    name: str
    kind: str


@dataclass(frozen=True)
class Choice:
    name: str
    kind: str
    arms: dict


@dataclass(frozen=True)
class Pound:
    name: str


class Parser:
    def __init__(self, text):
        self.text = text
        self.at = 0

    def error(self, message):
        raise CatalogError(f'{message} at column {self.at + 1}')

    def spaces(self):
        while self.at < len(self.text) and self.text[self.at].isspace():
            self.at += 1

    def token(self):
        start = self.at
        while self.at < len(self.text) and self.text[self.at] not in ',{} \t\r\n':
            self.at += 1
        return self.text[start:self.at]

    def sequence(self, nested=False, plural=None):
        nodes, literal = [], []

        def flush():
            if literal:
                nodes.append(Text(''.join(literal)))
                literal.clear()

        while self.at < len(self.text):
            c = self.text[self.at]
            if c == '}':
                if not nested:
                    self.error('unexpected closing brace')
                flush()
                self.at += 1
                return nodes
            if c == "'":
                nxt = self.text[self.at + 1:self.at + 2]
                if nxt == "'":
                    literal.append("'")
                    self.at += 2
                elif nxt and nxt in '{}#|':
                    self.at += 1
                    while self.at < len(self.text):
                        q = self.text[self.at]
                        self.at += 1
                        if q == "'":
                            if self.text[self.at:self.at + 1] == "'":
                                literal.append("'")
                                self.at += 1
                            else:
                                break
                        else:
                            literal.append(q)
                else:
                    literal.append(c)
                    self.at += 1
            elif c == '{':
                flush()
                self.at += 1
                nodes.append(self.argument(plural))
            elif c == '#' and plural is not None:
                flush()
                nodes.append(Pound(plural))
                self.at += 1
            else:
                literal.append(c)
                self.at += 1
        if nested:
            self.error('unclosed brace')
        flush()
        return nodes

    def argument(self, plural):
        self.spaces()
        name = self.token()
        if not NAME.fullmatch(name):
            self.error('invalid argument name')
        self.spaces()
        if self.text[self.at:self.at + 1] == '}':
            self.at += 1
            return Value(name, 'plain')
        if self.text[self.at:self.at + 1] != ',':
            self.error('expected comma or closing brace')
        self.at += 1
        self.spaces()
        kind = self.token()
        self.spaces()
        if kind == 'number':
            if self.text[self.at:self.at + 1] != '}':
                self.error('number takes no style')
            self.at += 1
            return Value(name, 'nat')
        if kind not in ('plural', 'select'):
            self.error('unsupported argument kind')
        if self.text[self.at:self.at + 1] != ',':
            self.error('expected comma before choices')
        self.at += 1
        arms = {}
        while True:
            self.spaces()
            if self.text[self.at:self.at + 1] == '}':
                self.at += 1
                break
            selector = self.token()
            if kind == 'plural':
                if re.fullmatch(r'=\d+', selector):
                    selector = '=' + str(int(selector[1:]))
                elif selector not in CATEGORIES:
                    self.error('invalid plural selector')
            elif not re.fullmatch(r'[A-Za-z0-9_-]+', selector):
                self.error('invalid select selector')
            if selector in arms:
                self.error('duplicate selector')
            self.spaces()
            if self.text[self.at:self.at + 1] != '{':
                self.error('expected choice body')
            self.at += 1
            arms[selector] = self.sequence(True, name if kind == 'plural' else plural)
        if 'other' not in arms:
            self.error('choice requires other')
        return Choice(name, kind, arms)


def unescape(text):
    out, at = [], 0
    escapes = {'n': '\n', '\\': '\\', 's': ' '}
    while at < len(text):
        c = text[at]
        at += 1
        if c == '\\':
            if at == len(text) or text[at] not in escapes:
                raise CatalogError('unknown or incomplete catalog escape')
            c = escapes[text[at]]
            at += 1
        out.append(c)
    return ''.join(out)


def parse_catalog(source, label):
    entries = {}
    for line_no, line in enumerate(source.splitlines(), 1):
        if not line.strip() or line.lstrip().startswith('#'):
            continue
        try:
            if '=' not in line:
                raise CatalogError('expected key = message')
            key, message = line.split('=', 1)
            key = key.strip()
            if not KEY.fullmatch(key) or any(s in KEYWORDS for s in key.split('.')):
                raise CatalogError(f'invalid key {key!r} (syntax or keyword segment)')
            if any(s[0].isdigit() for s in key.split('.')):
                raise CatalogError(f'key {key!r}: Bend name segments cannot begin with digits')
            if key in entries:
                raise CatalogError(f'duplicate key {key}')
            entries[key] = Parser(unescape(message.strip())).sequence()
        except CatalogError as exc:
            raise CatalogError(f'{label}:{line_no}: {exc}') from exc
    return entries


def uses(nodes):
    found = {}
    for node in nodes:
        if isinstance(node, (Value, Pound, Choice)):
            kind = 'nat' if isinstance(node, Pound) or node.kind == 'plural' else ('string' if node.kind == 'select' else node.kind)
            found.setdefault(node.name, set()).add(kind)
        if isinstance(node, Choice):
            for body in node.arms.values():
                for name, kinds in uses(body).items():
                    found.setdefault(name, set()).update(kinds)
    return found


def validate(catalogs):
    if 'en' not in catalogs or 'ja' not in catalogs:
        raise CatalogError('en.msg and ja.msg are required')
    if set(catalogs) - {tag for tag, _ in LOCALES}:
        raise CatalogError('unsupported locale: add its Loc variant and generator arm first')
    source, signatures = catalogs['en'], {}
    for tag, catalog in catalogs.items():
        extra = sorted(set(catalog) - set(source))
        if extra:
            raise CatalogError(f'{tag}: keys absent from en: {extra}')
    for key, nodes in source.items():
        combined = uses(nodes)
        for tag, catalog in catalogs.items():
            if key not in catalog:
                continue
            local = uses(catalog[key])
            if set(local) != set(combined):
                raise CatalogError(f'{tag}:{key}: placeholder mismatch: expected {sorted(combined)}, got {sorted(local)}')
            for name, kinds in local.items():
                combined[name].update(kinds)
        for name, kinds in combined.items():
            if {'nat', 'string'} <= kinds:
                raise CatalogError(f'{key}: kind mismatch for {name}')
        signatures[key] = {name: 'Nat' if 'nat' in kinds else 'String' for name, kinds in combined.items()}
    return signatures


def literal(text):
    return json.dumps(text, ensure_ascii=False)


def compile_nodes(nodes, params, signature, locale):
    pieces = []
    for node in nodes:
        if isinstance(node, Text):
            pieces.append(literal(node.value))
        elif isinstance(node, (Value, Pound)):
            arg = params[node.name]
            pieces.append(f'I.I18n.num({locale}, {arg})' if signature[node.name] == 'Nat' else arg)
        else:
            arms = {key: compile_nodes(body, params, signature, locale) for key, body in node.arms.items()}
            other = arms['other']
            arg = params[node.name]
            if node.kind == 'plural':
                expr = f'I.I18n.pick(I.I18n.plural({locale}, {arg}), ' + ', '.join(arms.get(c, other) for c in CATEGORIES) + ')'
                for key in reversed(list(arms)):
                    if key.startswith('='):
                        # Use digit arithmetic, avoiding out-of-range U32 literals.
                        n = '0n'
                        for digit in key[1:]:
                            n = f'Nat.add(Nat.mul({n}, 10n), {digit}n)'
                        expr = f'I.I18n.exact({arg}, {n}, {arms[key]}, {expr})'
            else:
                expr = other
                for key in reversed(list(arms)):
                    if key != 'other':
                        expr = f'I.I18n.sel({arg}, {literal(key)}, {arms[key]}, {expr})'
            pieces.append(expr)
    return ' ++ '.join(pieces) or '""'


def collision_check(names):
    seen = {}
    for name in names:
        mangled = name.replace('.', '_').upper()
        if mangled in seen:
            raise CatalogError(f'Bend mangling collision: {seen[mangled]} and {name}')
        seen[mangled] = name


def require_complete(catalogs):
    missing = [f'{tag}:{key}' for tag, _ in LOCALES
               for key in sorted(set(catalogs['en']) - set(catalogs[tag]))]
    if missing:
        raise CatalogError('missing translations: ' + ', '.join(missing))


def generate(catalogs, i18n_import='./i18n.bend'):
    signatures = validate(catalogs)
    keys = sorted(catalogs['en'])
    names, lines = [], ['# GENERATED by scripts/i18n-gen.py from i18n/*.msg. Do not edit.', '# Regenerate: python3 scripts/i18n-gen.py', 'import Base', f'import {i18n_import} as I', '']
    for key in keys:
        sig = signatures[key]
        params = {name: f'arg{i}' for i, name in enumerate(sig)}
        name = 'T.' + key
        names.append(name)
        arms = []
        for tag, variant in LOCALES:
            # An absent translation gets the English arm, including plural rules.
            actual = variant if key in catalogs[tag] else 'En'
            nodes = catalogs[tag].get(key, catalogs['en'][key])
            arms.append((variant, compile_nodes(nodes, params, sig, f'I.{actual}{{}}')))
        # Locale arms are exclusive; count uses within each expression, not across arms.
        counts = {param: max(len(re.findall(r'\b' + param + r'\b', expr)) for _, expr in arms) for param in params.values()}
        args = ''.join(f', {"+" if counts[params[a]] > 1 else ""}{params[a]}: {kind}' for a, kind in sig.items())
        lines.extend([f'def {name}(l: I.Loc{args}) -> String:', '  match l:'])
        for variant, expr in arms:
            lines.extend([f'    case I.{variant}{{}}:', '      ' + expr])
        lines.append('')
    names.extend(['T.keys', 'T.missing', 'T.plain'])
    lines.extend(['def T.keys() -> List<&2, String>:', '  [' + ', '.join(map(literal, keys)) + ']', '', 'def T.missing(l: I.Loc) -> List<&2, String>:', '  match l:'])
    for tag, variant in LOCALES:
        missing = [k for k in keys if k not in catalogs[tag]]
        lines.extend([f'    case I.{variant}{{}}:', '      [' + ', '.join(map(literal, missing)) + ']'])
    lines.append('')
    plain = [k for k in keys if not signatures[k]]

    def tree(items, path='n'):
        if not items:
            return None
        mid = len(items) // 2
        key = items[mid]
        left = tree(items[:mid], path + 'l')
        right = tree(items[mid + 1:], path + 'r')
        name = 'T.plain.' + path
        names.extend([name + '.cmp', name])
        lines.extend([f'def {name}.cmp(r: (String & String) & Cmp, l: I.Loc) -> Maybe<&2, String>:', '  ((key, _), c) = r', '  match c:', '    case LT{}:', f'      {left}(l, key)' if left else '      None{}', '    case EQ{}:', f'      Some{{T.{key}(l)}}', '    case GT{}:', f'      {right}(l, key)' if right else '      None{}', '', f'def {name}(l: I.Loc, key: String) -> Maybe<&2, String>:', f'  {name}.cmp(String.cmp(key, {literal(key)}), l)', ''])
        return name

    root = tree(plain)
    lines.extend(['def T.plain(l: I.Loc, key: String) -> Maybe<&2, String>:', f'  {root}(l, key)' if root else '  None{}', ''])
    collision_check(names)
    return '\n'.join(lines)


def fallback_fixture():
    catalogs = {
        'en': parse_catalog('common.save = Save\nfiles.count = {n, plural, one {# file} other {# files}}', 'fallback/en'),
        'ja': {},
    }
    return generate(catalogs, '../../src/core/i18n.bend').replace(
        'from i18n/*.msg.', 'from built-in fallback fixtures.')


def self_test():
    total = 0

    def case(name, en, ja=None, error=False, contains=None, strict=False):
        nonlocal total
        try:
            catalogs = {'en': parse_catalog(en, 'en'), 'ja': parse_catalog(en if ja is None else ja, 'ja')}
            if strict:
                require_complete(catalogs)
            output = generate(catalogs)
        except CatalogError:
            if not error:
                raise AssertionError(f'{name}: unexpectedly rejected')
        else:
            if error:
                raise AssertionError(f'{name}: expected rejection')
            if contains is not None and contains not in output:
                raise AssertionError(f'{name}: missing {contains!r}')
        total += 1
        print('ok ' + name)

    case('bad key', 'bad_key = x', error=True)
    case('keyword segment', 'thread.when = x', error=True)
    case('numeric Bend segment', 'thread.1 = x', error=True)
    case('unknown placeholder', 'x = {title}', 'x = {wrong}', error=True)
    case('missing placeholder', 'x = {title}', 'x = x', error=True)
    case('kind mismatch', 'x = {n, number}', 'x = {n, select, other {x}}', error=True)
    case('within-locale kind mismatch', 'x = {n, number}{n, select, other {x}}', error=True)
    case('missing other', 'x = {n, plural, one {x}}', error=True)
    case('select missing other', 'x = {s, select, yes {x}}', error=True)
    case('unbalanced braces', 'x = {name', error=True)
    case('extra brace', 'x = x}', error=True)
    case('duplicate selector', 'x = {n, plural, =0 {a} =00 {b} other {c}}', error=True)
    case('bad category', 'x = {n, plural, bogus {x} other {y}}', error=True)
    case('unsupported kind', 'x = {n, date}', error=True)
    case('duplicate key', 'x = a\nx = b', error=True)
    case('extra ja key', 'x = a', 'x = a\ny = b', error=True)
    case('mangling collision', 'keys = x', error=True)
    case('helper collision', 'plain.n = x', error=True)
    case('apostrophe quoting', "x = don't '' '{title}' '#|'", contains=literal("don't ' {title} #|"))
    case('quoted plural pound', "x = {n, plural, other {'#' #}}", contains='"# " ++ I.I18n.num')
    case('pound outside plural is literal', 'x = #42', contains='"#42"')
    case('catalog escapes', r'x = \sa\n\\\s', contains=literal(' a\n\\ '))
    case('bad escape', r'x = \q', error=True)
    case('plain promoted to Nat', 'x = {n, plural, one {# file} other {# files}}', 'x = {n}件', contains='arg0: Nat')
    case('Nat inferred across locales', 'x = {n}', 'x = {n, number}', contains='arg0: Nat')
    case('nested select keeps outer pound', 'x = {n, plural, other {{who, select, you {# yours} other {# theirs}}}}', contains='I.I18n.sel')
    case('nested plural', 'x = {n, plural, other {{m, plural, other {#}} #}}', contains='I.I18n.num(I.En{}, arg1)')
    case('fallback', 'x = fallback', '', contains='"fallback"')
    case('release rejects missing translation', 'x = source', '', error=True, strict=True)
    case('release accepts complete translation', 'x = source', 'x = 訳', strict=True)
    fallback = generate({'en': parse_catalog('x = {n, plural, one {# item} other {# items}}', 'en'), 'ja': {}})
    en_arm = fallback.split('case I.En{}:\n', 1)[1].splitlines()[0]
    ja_arm = fallback.split('case I.Ja{}:\n', 1)[1].splitlines()[0]
    assert en_arm == ja_arm, 'plural fallback must preserve English grammar'
    total += 1
    print('ok plural fallback preserves English grammar')
    print(f'i18n self-test: {total} cases passed')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--check', action='store_true')
    modes.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    try:
        if args.self_test:
            self_test()
            return 0
        catalogs = {p.stem: parse_catalog(p.read_text(encoding='utf-8'), str(p.relative_to(ROOT))) for p in sorted((ROOT / 'i18n').glob('*.msg'))}
        output = generate(catalogs)
        missing = sorted(set(catalogs['en']) - set(catalogs['ja']))
        print(f'i18n: {len(catalogs["en"])} keys, ja missing {len(missing)}')
        for key in missing:
            print(f'warning: ja missing {key}; using English')
        if args.check:
            require_complete(catalogs)
        outputs = {
            ROOT / 'src/core/msgs.bend': output,
            ROOT / 'test/fixtures/i18n_fallback.bend': fallback_fixture(),
        }
        for target, text in outputs.items():
            if args.check:
                if not target.exists() or target.read_bytes() != text.encode('utf-8'):
                    raise CatalogError(f'{target.relative_to(ROOT)} is stale; run python3 scripts/i18n-gen.py')
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(text, encoding='utf-8')
        return 0
    except (CatalogError, UnicodeError, OSError, AssertionError) as exc:
        print(f'i18n: {exc}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
