// The JS twin of adopt.c: the JS host starts no processes, so it keeps no
// record and adopts nothing.
function adopt_record(path) {
  return { $: "Unit" };
}

function adopt_keep(thread, pid, socket) {
  return socket;
}

function adopt_note(thread, pid) {
  return { $: "Unit" };
}

function adopt_drop(thread, pid) {
  return { $: "Unit" };
}

function adopt_ident(pid) {
  return "";
}

function adopt_take(fd) {
  return io_fail(9);
}

function adopt_stop(pid, ident) {
  return { $: "False" };
}

function adopt_reap() {
  return 0;
}

io_eff(CID(Adopt.record), adopt_record);
io_eff(CID(Adopt.reap), adopt_reap);
io_eff(CID(Adopt.keep), adopt_keep);
io_eff(CID(Adopt.note), adopt_note);
io_eff(CID(Adopt.drop), adopt_drop);
io_eff(CID(Adopt.ident), adopt_ident);
io_eff(CID(Adopt.take), adopt_take);
io_eff(CID(Adopt.stop), adopt_stop);
