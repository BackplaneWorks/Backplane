import Base
import ./liveness_test.bend as L
import ./lease_heartbeat_test.bend as K
import ./liveness_recovery_test.bend as R
import ./liveness_ui_test.bend as U
import ./restart_retry_test.bend as RR
import ./watchdog_test.bend as W

# Compile the shared hub/client imports once for the required fast gate.
def main() -> IO(Unit):
  do IO<Unit>:
    L.main()
    K.main()
    R.main()
    U.main()
    RR.main()
    W.main()
