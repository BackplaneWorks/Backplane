// Pure literal placeholder interpolation for i18n messages.
// Single-pass regex + function replacer guarantees:
// 1. Replacement values containing `$&`, `$$`, `` $` ``, or `$'` are inserted verbatim.
// 2. A placeholder token inside one variable's value is never substituted by another variable.
export function interpolate(template: string, vars?: Record<string, string | number>): string {
  if (!vars) return template;
  return template.replace(/\{([a-zA-Z0-9_.-]+)\}/g, (match, key: string) =>
    Object.prototype.hasOwnProperty.call(vars, key) ? String(vars[key]) : match,
  );
}
