import Base
import ./check.bend as C
import ./thin_terminal_input_test.bend as F
import ../src/core/client.bend as U
import ../src/core/json.bend as J

# A review page asked of the hub reads until its report comes. A request
# that fails (no report) used to leave it reading for ever; now the page
# stops and says why, and other replies leave it as it was.

def asked() -> U.Ui:
  U.Act.ui(U.Rp.ask.bom(U.Ui.info_set(F.ui(), "rv.page:a", "bom")))

def answered(+reply: String) -> U.Ui:
  U.Rp.reply(False{}, asked(), F.obj(reply))

def busy(+u: U.Ui) -> String:
  U.Ui.scratch(u, "@rp.busy:a")

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("asking reads the page", busy(asked()), "bom")
    C.Check.str("its failure ends the reading", busy(answered("{\"t\":\"reply\",\"id\":100,\"ok\":false,\"error\":\"git said no\"}")), "")
    C.Check.str("and the page says why", U.Ui.info(answered("{\"t\":\"reply\",\"id\":100,\"ok\":false,\"error\":\"git said no\"}"), "rp.err:a:bom"), "git said no")
    C.Check.str("another request's failure leaves it reading", busy(answered("{\"t\":\"reply\",\"id\":7,\"ok\":false,\"error\":\"other\"}")), "bom")
    C.Check.str("an ok without a report leaves it reading", busy(answered("{\"t\":\"reply\",\"id\":100,\"ok\":true}")), "bom")
    C.Check.str("its report ends the reading as before", busy(U.Rp.reply(True{}, asked(), F.obj("{\"t\":\"reply\",\"id\":100,\"ok\":true,\"result\":{\"report\":{\"thread\":\"a\",\"page\":\"bom\",\"md\":\"x\",\"err\":\"\"}}}"))), "")
