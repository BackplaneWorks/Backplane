function fault_raise() {
  if (process.env.BACKPLANE_TEST_FAULT === 'range') {
    function diagnostic_fault_origin() {
      throw new RangeError('PRIVATE_PROMPT CAD_CONTENT credential=SECRET');
    }
    diagnostic_fault_origin();
  }
  if (process.env.BACKPLANE_TEST_FAULT === 'exit') process.exit(23);
  return { $: 'Unit' };
}
io_eff(CID(Fault.raise), fault_raise);
function fault_ready() {
  const file = process.env.BACKPLANE_TEST_FAULT_FILE;
  return Boolean(file && require('fs').existsSync(file));
}
io_eff(CID(Fault.ready), fault_ready);
