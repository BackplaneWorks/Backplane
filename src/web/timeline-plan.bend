import ../core/json.bend as J
import ../core/timeline-virtual.bend as V

# Separate entry point so the DOM shim can import this pure planner without
# owning or recompiling the application's state/action interface.
def timeline_plan(input: J.Json) -> J.Json:
  V.plan(input)
