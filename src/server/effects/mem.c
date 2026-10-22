// Mem.settle (host.bend): nothing to settle in the native build (no GC heap
// holds the thin index's build); the line to log is "".

#ifdef CID(Mem.settle)

Term mem_settle_run(Env e, Term* f, IoWork* w) {
  (void)f;
  return term_pak(CID(SNil), 0);
}

static void __attribute__((constructor)) mem_settle_use(void) {
  io_eff(CID(Mem.settle), mem_settle_run);
}

#endif
