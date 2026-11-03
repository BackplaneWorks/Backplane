// Private-home account fallback simulator. Never authenticates or calls a provider.
const fs = require('node:fs'), path = require('node:path'), rl = require('node:readline');
const [provider, ...args] = process.argv.slice(2);
const emit = value => process.stdout.write(JSON.stringify(value) + '\n');
const store = process.env.CLAUDE_CONFIG_DIR || '';
const owner = store ? path.basename(store) : 'Default';
const session = 'synthetic-' + owner + '-session';
const audit = row => fs.appendFileSync(process.env.BP_ACCOUNT_FALLBACK_AUDIT, JSON.stringify({provider, owner, ...row}) + '\n');
if (args.includes('--version')) { console.log(provider === 'claude' ? '2.1.300 (synthetic)' : 'codex-cli 0.165.0'); process.exit(0); }
if (provider === 'claude' && args[0] === 'auth') {
  emit({loggedIn: true, authMethod: 'claude.ai', email: 'synthetic@example.test', orgUuid: 'synthetic', subscriptionType: 'fixture'});
  process.exit(0);
}
if (provider === 'claude' && args.includes('--input-format')) {
  const resumed = args.includes('--resume') ? args[args.indexOf('--resume') + 1] : '';
  if (resumed && resumed !== session) { audit({kind: 'wrong-session', resumed}); process.exit(94); }
  rl.createInterface({input: process.stdin}).on('line', line => {
    const q = JSON.parse(line);
    if (q.type === 'control_request') {
      emit({type: 'control_response', response: {subtype: 'success', request_id: q.request_id,
        response: {models: [{value: 'default', displayName: 'Synthetic Claude', supportedEffortLevels: ['low', 'high']}]}}});
      return;
    }
    if (q.type !== 'user') return;
    const text = typeof q.message?.content === 'string' ? q.message.content : JSON.stringify(q.message?.content);
    audit({kind: 'turn', text, session, resumed});
    emit({type: 'system', subtype: 'init', session_id: session, model: 'default', tools: [], mcp_servers: []});
    const control = JSON.parse(fs.readFileSync(process.env.BP_ACCOUNT_FALLBACK_CONTROL, 'utf8'));
    if (owner === 'Default' || control.allClaudeLimited) {
      emit({type: 'result', is_error: true, errors: ['Weekly usage limit reached'], resets_at: control.reset,
        session_id: session, num_turns: 1});
    } else {
      emit({type: 'assistant', session_id: session, message: {id: 'synthetic-answer', role: 'assistant', content: [{type: 'text', text: 'Synthetic request completed'}]}});
      emit({type: 'result', is_error: false, result: 'Synthetic request completed', session_id: session, num_turns: 1});
    }
  });
} else if (provider === 'codex' && args.includes('exec')) {
  audit({kind: 'turn', text: args.at(-1), session: 'synthetic-codex'});
  emit({type: 'thread.started', thread_id: 'synthetic-codex'});
  emit({type: 'item.completed', item: {type: 'agent_message', text: 'Synthetic cross-provider completion'}});
  emit({type: 'turn.completed', usage: {input_tokens: 1, output_tokens: 1}});
} else if (provider === 'claude' && args.includes('-p')) {
  process.stdin.resume(); process.stdin.on('end', () => { emit({type: 'result', is_error: false, result: 'Synthetic title'}); process.exit(0); });
}
