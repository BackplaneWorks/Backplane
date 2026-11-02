import Base

# One installation-wide choice covers every telemetry category. Missing values
# select On in the disclosure, but never authorize collection or transmission.
# An older explicit Off or expanded-metrics decline always remains Off.
def Choice.on(+share: String, usage: String) -> Bool:
  Bool.and(Bool.or(String.is_empty(share), String.eq(share, "on")), Bool.not(String.eq(usage, "0")))

def Choice.answered(+share: String, asked: String, +usage: String) -> Bool:
  Bool.or(String.eq(share, "off"), Bool.or(String.eq(usage, "0"),
    Bool.and(String.eq(asked, "1"), String.eq(usage, "1"))))

def Choice.allowed(+share: String, asked: String, +usage: String) -> Bool:
  Bool.and(Choice.on(share, usage), Bool.and(String.eq(asked, "1"), String.eq(usage, "1")))
