// A diagnostic report is never a performance acceptance result. Requirements
// come from the selected closed schema, never from the report's own keys.
export type PhaseChecks = { name: string; checks: Record<string, boolean | null> };
export type GateProfile = 'native-workload' | 'bound-evidence';
const workloadCommon = ['all agents streaming','frame p99<=8ms','no frame>16ms','agent_line p99<=2ms'];
const evidenceCommon = ['phase >=20 seconds','valid complete spans','all ten streams sustained',
  'at least 300 frames','frame p99<=8ms','no streaming frame>16ms','agent_line p99<=2ms',
  'supplied content matches phase window/budget'];
const workload = {
  bg: workloadCommon, fg: workloadCommon,
  type: [...workloadCommon,'input p99<=16ms','key>frame p99<=16ms'],
  open: [...workloadCommon,'all ten opens sampled','open<=50ms'],
  scroll: workloadCommon,
  plane: [...workloadCommon,'Status tab observed'],
  normal: [...workloadCommon,'Status tab observed','input p99<=16ms','key>frame p99<=16ms'],
};
const evidence = {
  'continuous-scroll': [...evidenceCommon,'content-correlated native cadence','cadence covers full phase'],
  input: [...evidenceCommon,'exact demanded content within budget','input p99<=16ms'],
  'thread-switch': [...evidenceCommon,'exact demanded content within budget','ten actual drawn opens<=50ms'],
  background: evidenceCommon,
};
const workloadOptional = {
  idle:['frame p99<=8ms','no frame>16ms'],
  mail:[...workloadCommon,'all ten opens sampled','open<=50ms','Mailroom and ten conversation selections visually verified'],
};
const evidenceOptional = {normal:evidenceCommon,fg:evidenceCommon,plane:evidenceCommon,mail:evidenceCommon};
export function performanceVerdict(phases: PhaseChecks[], workloadValid: boolean, diagnostic = false,
    profile: GateProfile = 'native-workload') {
  const failures: string[] = [];
  if (!workloadValid) failures.push('invalid workload');
  const validProfile = profile === 'native-workload' || profile === 'bound-evidence';
  if (!validProfile) failures.push('unknown gate profile');
  const required:Record<string,readonly string[]> = profile === 'bound-evidence' ? evidence : workload;
  const optional:Record<string,readonly string[]> = profile === 'bound-evidence' ? evidenceOptional : workloadOptional;
  const measured = Array.isArray(phases) ? phases : [];
  if (!measured.length) failures.push('no measured phases');
  const names = new Set<string>();
  for (const phase of measured) {
    if (!phase || typeof phase.name !== 'string') { failures.push('invalid phase'); continue; }
    if (names.has(phase.name)) failures.push(`duplicate phase: ${phase.name}`);
    names.add(phase.name);
    const requirements = Object.hasOwn(required,phase.name) ? required[phase.name]
      : Object.hasOwn(optional,phase.name) ? optional[phase.name] : undefined;
    if (!requirements) failures.push(`unknown phase: ${phase.name}`);
    const checks = phase.checks && typeof phase.checks === 'object' && !Array.isArray(phase.checks) ? phase.checks : {};
    const entries = Object.entries(checks);
    if (!entries.length) failures.push(`${phase.name}: no checks`);
    for (const name of requirements ?? []) if (!Object.hasOwn(checks,name)) failures.push(`${phase.name}: ${name} (omitted evidence)`);
    for (const [name,passed] of entries) if (passed !== true) failures.push(`${phase.name}: ${name} (${passed === null ? 'missing evidence' : 'failed'})`);
  }
  for (const name of Object.keys(required)) if (!names.has(name)) failures.push(`required phase missing: ${name}`);
  return { mode: diagnostic ? 'diagnostic' : 'acceptance', accepted: !diagnostic && failures.length === 0,
    failures, exitCode: !validProfile || !workloadValid || (!diagnostic && failures.length > 0) ? 1 : 0 };
}
