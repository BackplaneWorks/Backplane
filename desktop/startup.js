const status = document.getElementById('status');
const retry = document.getElementById('retry');
window.backplaneDesktop.onConnection(({phase,message}) => {
  status.textContent = (message.startsWith('Building the database') || message === 'Starting your service…') ? `${message}. This can take a few minutes. Keep this window open.` : message;
  retry.hidden = phase !== 'error';
});
retry.addEventListener('click', () => {
  retry.hidden = true; status.textContent = 'Connecting to your service…';
  window.backplaneDesktop.retry().catch(() => { status.textContent = 'Could not connect.'; retry.hidden = false; });
});
