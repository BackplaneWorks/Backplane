const status = document.getElementById('status');
const retry = document.getElementById('retry');
const update = ({phase, message}) => { status.textContent = message; retry.hidden = phase !== 'error'; };
retry.addEventListener('click', () => { retry.hidden = true; void window.backplaneDesktop.retry(); });
window.backplaneDesktop.onConnection(update);
// Main outlives this document, including an error and a press on Reload.
const current = window.backplaneDesktop.connectionState();
if (current) update(current);
