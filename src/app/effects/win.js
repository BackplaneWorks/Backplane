// The native window has no JS twin: the app ships as a native binary.
function win_open(title, w, h) {
  return io_fail(38);
}
function win_words(win) {
  return io_tup(win, { $: "Nil" });
}
function win_present(win, image) {
  return io_tup(win, image);
}
function win_show(win, image, prev) {
  return io_tup(win, io_tup(image, prev));
}
function win_copy(win, text) {
  return win;
}
function win_paste(win) {
  return win;
}
function win_size(win) {
  return io_tup(win, io_tup(0, 0));
}
function win_twin(win) {
  return io_tup(win, win);
}
function win_title(win, title) {
  return win;
}
function win_pasted(win) {
  return io_tup(win, "");
}
function win_dropped(win) {
  return io_tup(win, "");
}

io_eff(CID(Win.open), win_open);
io_eff(CID(Win.words), win_words);
io_eff(CID(Win.present), win_present);
io_eff(CID(Win.show), win_show);
io_eff(CID(Win.copy), win_copy);
io_eff(CID(Win.paste), win_paste);
io_eff(CID(Win.pasted), win_pasted);
io_eff(CID(Win.dropped), win_dropped);
io_eff(CID(Win.size), win_size);
io_eff(CID(Win.twin), win_twin);
io_eff(CID(Win.title), win_title);

function win_pasted_image(win) {
  return io_tup(win, io_tup("", io_tup(0, "")));
}
io_eff(CID(Win.pasted_image), win_pasted_image);

function win_ime(win) {
  return io_tup(win, 0);
}
function win_ime_reset(win) {
  return win;
}
function win_ime_spot(win, x, y) {
  return win;
}
function win_ime_inject(win, kind, text) {
  return win;
}
io_eff(CID(Win.ime), win_ime);
io_eff(CID(Win.ime_reset), win_ime_reset);
io_eff(CID(Win.ime_spot), win_ime_spot);
io_eff(CID(Win.ime_inject), win_ime_inject);
function win_payload(win, id) {
  return io_tup(win, io_tup("", ""));
}
io_eff(CID(Win.payload), win_payload);

function win_xid(win) { return io_tup(win, 0); }
io_eff(CID(Win.xid), win_xid);
