const status = document.getElementById('status');
const retry = document.getElementById('retry');
const update = ({phase,message}) => {
  status.textContent = message;
  retry.hidden = phase !== 'error';
};
window.backplaneDesktop.onConnection(update);
// Main outlives this document, including an error and a press on Reload.
const current = window.backplaneDesktop.connectionState();
if (current) update(current);
retry.addEventListener('click', () => {
  retry.hidden = true; status.textContent = 'Connecting to your service…';
  window.backplaneDesktop.retry().catch(() => { status.textContent = 'Could not connect.'; retry.hidden = false; });
});
