const reference = (kind, id, name = '') => id ? { id, name, url: 'backplane://' + kind + '/' + encodeURIComponent(id) } : null;

// Resolve the application source, never the project being viewed. More than
// one matching checkout requires the person's deliberate project selection.
export function backplaneProject(projects = []) {
  const basename = p => (p.root || '').replace(/\\/g, '/').replace(/\/+$/, '').split('/').at(-1)?.toLowerCase();
  const canonical = projects.filter(p => basename(p) === 'backplane-bend');
  return canonical.length === 1 ? canonical[0] : null;
}

export function improvementOrigin({ session = {}, doc = document } = {}) {
  const main = doc.querySelector('main.thread-view');
  const thread = main?.dataset.thread || session.sel || '';
  // Bots.create / uber_join / spawn generate the original b… and t… IDs
  // together. A direct selection of that chat has no bot route; require the
  // matching rendered bot as well before recording its reference.
  const originalBot = thread.startsWith('t') ? [...doc.querySelectorAll('[data-on-click="bot"],[data-on-click="uber"]')].find(n => n.dataset.value === 'b' + thread.slice(1)) : null;
  const bot = session.route?.bot || doc.querySelector('.bot-row.on')?.dataset.value || originalBot?.dataset.value || '';
  return {
    thread: main?.dataset.thread || (!bot ? session.sel : '') || '',
    project: main?.dataset.project || '',
    bot,
    name: (main?.querySelector('.bot-title,.title-box .title') || doc.querySelector('.topbar .title-box .title,.topbar .crumb:last-child'))?.textContent?.trim() || '',
    view: doc.querySelector('.viewer-tabs .on')?.textContent?.trim() || session.route?.tab || 'Thread',
    path: session.info?.['view.path:' + (main?.dataset.thread || session.sel || '')] || '',
  };
}

export function improvementContext(origin, result) {
  const project = result.projects?.find(p => p.id === (result.project || origin.project));
  return {
    app: 'Backplane',
    sourceThread: reference('thread', origin.thread || result.thread, origin.name),
    sourceBot: reference('bot', origin.bot, origin.bot ? origin.name : ''),
    sourceProject: project ? { ...reference('project', project.id, project.title), root: project.root } : null,
    view: origin.view, path: origin.path,
    implementationProject: backplaneProject(result.projects),
    projects: result.projects || [],
  };
}
