const status = document.getElementById('status');
// Main reconnects by itself (it retries until the service answers): this
// page only says so, and has nothing to press.
const update = ({message}) => { status.textContent = message; };
window.backplaneDesktop.onConnection(update);
// Main outlives this document, including an error and a press on Reload.
const current = window.backplaneDesktop.connectionState();
if (current) update(current);
