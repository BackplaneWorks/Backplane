import { newCreationKey, improvementThread } from './improvements-flow.js';
// Normal visible project thread, normal account binding, normal provider and
// permissions. No hidden agent, credential lookup or GitHub publishing path.
export class ReportWriter {
  constructor({ rpc, save, upload, identity = () => 'fixture', expectedScope = identity }) { Object.assign(this, { rpc, save, upload, identity, expectedScope });this.busy = false; }
  guard(expected) { if (!expected || this.identity() !== expected) throw Error('The hub identity changed. Reconnect to the same hub before retrying this report.'); }
  async start(draft, prompt, { image = null, project = draft.writerProject } = {}) {
    if (this.busy) return false;
    this.busy = true;
    try {
      if (typeof prompt !== 'string' || new TextEncoder().encode(prompt).length > 125000) throw Error('The report description is too long for the agent. Shorten it or fill the template locally.');
      if (!draft.writer && (!project || project === 'scratch')) throw Error('Choose a project for the report agent. You can fill the template without an agent.');
      const expected = this.expectedScope();this.guard(expected);
      if (!draft.writer) draft.writer = { key: newCreationKey(), prompt, withImage: !!image, image, scope: expected, project };
      const w = draft.writer;
      if (!w.project || w.project === 'scratch') throw Error('Open the original report-writing thread, or choose a project to start a new report. Your saved report is kept.');
      if (w.scope && w.scope !== expected) throw Error('This report belongs to another hub identity.');
      w.scope ||= expected;
      await this.save(draft);this.guard(expected);
      const result = await this.rpc.request('thread.create', { project: w.project, title: 'Write an issue report', creationKey: w.key });
      this.guard(expected);w.thread = improvementThread(w.key);
      if (result?.thread !== w.thread) throw Error('The hub did not confirm the report thread. Retry the same writer.');
      await this.save(draft);this.guard(expected);
      if (w.binding === undefined) {
        const context = await this.rpc.request('improvements.context', { sourceThread: w.thread });
        this.guard(expected);if (context?.thread !== w.thread || typeof context.accountBinding !== 'string') throw Error('The hub did not confirm the report account. Retry.');
        w.binding = context.accountBinding;await this.save(draft);this.guard(expected);
      }
      if (w.withImage && !w.path) { this.guard(expected);w.path = await this.upload(w.thread, w.key, w.image);this.guard(expected);await this.save(draft);this.guard(expected); }
      w.message ||= w.prompt + (w.path ? '\n\n[attachment: ' + w.path + ']' : '');
      await this.save(draft);this.guard(expected);
      await this.rpc.request('turn.start', { thread: w.thread, text: w.message, msg: 'issue-writer-' + w.key, accountBinding: w.binding });
      this.guard(expected);w.started = true;await this.save(draft);return true;
    } finally { this.busy = false; }
  }
  async load(draft) {
    if (this.busy || !draft.writer?.thread) return null;
    this.busy = true;
    try {
      const expected = draft.writer.scope || this.expectedScope();this.guard(expected);
      const result = await this.rpc.request('improvements.report', { thread: draft.writer.thread, accountBinding: draft.writer.binding });
      this.guard(expected);
      if (result?.thread !== draft.writer.thread) throw Error('The hub returned a different report.');
      if (!result.done) throw Error('The report agent is still working. Open its thread for questions, then load the report when it finishes.');
      if (typeof result.body !== 'string' || !result.body.trim() || new TextEncoder().encode(result.body).length > 120000) throw Error('No complete report is available. Open the report thread and check the agent response.');
      return result.body;
    } finally { this.busy = false; }
  }
}
