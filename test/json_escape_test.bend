import Base
import ./check.bend as C
import ../src/core/json.bend as J

def parsed_show(result: Maybe<&2, J.Json>) -> String:
  match result:
    case None{}: "<bad>"
    case Some{value}: J.Json.show(value)

def parse_show(text: String) -> String:
  parsed_show(J.Json.parse(text))

def main() -> IO(Unit):
  do IO<Unit>:
    C.Check.str("Unicode invalid hex rejected", parse_show("\"\\uZZZZ\""), "<bad>")
    C.Check.str("Unicode short escape rejected", parse_show("\"\\u123\""), "<bad>")
    C.Check.str("Unicode unfinished escape rejected", parse_show("\"\\u123"), "<bad>")
    C.Check.str("Unicode escape lower digit bound", parse_show("\"\\u/000\""), "<bad>")
    C.Check.str("Unicode escape upper digit gap", parse_show("\"\\u:000\""), "<bad>")
    C.Check.str("Unicode escape uppercase bound", parse_show("\"\\uG000\""), "<bad>")
    C.Check.str("Unicode escape lowercase bound", parse_show("\"\\ug000\""), "<bad>")
    C.Check.str("Unicode error stays rejected through suffix", parse_show("\"\\u0G01\" null"), "<bad>")
    C.Check.str("Unicode error stays rejected in array", parse_show("[\"\\u0G01\",1]"), "<bad>")
    C.Check.str("Unicode error after high surrogate", parse_show("\"\\ud83d\\u0G01\""), "<bad>")
    C.Check.str("Unicode valid mixed case", parse_show("\"\\u00aF\""), "\"¯\"")
    C.Check.str("Unicode valid pair retained", parse_show("\"\\uD83D\\udc69\""), "\"👩\"")
    C.Check.str("Unicode lone surrogate normalization retained", parse_show("\"\\uD800\""), "\"�\"")
