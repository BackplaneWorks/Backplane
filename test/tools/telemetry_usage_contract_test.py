import ast
from pathlib import Path
from types import SimpleNamespace
import unittest

from telemetry_usage_contract import CLIENT_USAGE_COUNTS, expected_counts


class UsageContract(unittest.TestCase):
    def test_all_twelve_fixed_integer_fields_including_boards(self):
        actual = expected_counts(5)
        self.assertEqual(tuple(actual), (
            "threads_created", "user_messages", "assistant_messages", "bot_messages", "bot_wakes",
            "input_tokens", "cache_read_tokens", "cache_write_tokens", "output_tokens", "usage_turns",
            "boards_created", "boards_ordered",
        ))
        self.assertEqual(actual["boards_created"], 5)
        self.assertEqual(actual["boards_ordered"], 5)
        self.assertTrue(all(type(n) is int for n in actual.values()))
        self.assertEqual(sum(actual[k] for k in ("input_tokens", "cache_read_tokens", "cache_write_tokens", "output_tokens")), 950)

    def test_zero_fixture_preserves_all_optional_keys(self):
        self.assertEqual(expected_counts(0), dict.fromkeys(CLIENT_USAGE_COUNTS, 0))

    def test_paired_template_does_not_truncate_to_receiver_legacy_constant(self):
        tree = ast.parse(Path(__file__).with_name("telemetry_usage_e2e.py").read_text())
        expected = next(n for n in ast.walk(tree) if isinstance(n, ast.FunctionDef) and n.name == "expected")
        # Execute the actual pure fixture function with the receiver's legacy
        # ten-field constant, without importing Flask or starting any server.
        scope = {"telemetry": SimpleNamespace(USAGE_COUNTS=CLIENT_USAGE_COUNTS[:10]),
                 "expected_counts": expected_counts}
        module = ast.Module(body=[expected], type_ignores=[])
        exec(compile(ast.fix_missing_locations(module), "<paired-fixture>", "exec"), scope)
        actual = scope["expected"](5)
        self.assertEqual(len(actual), 12)
        self.assertEqual(actual["boards_created"], 5)
        self.assertEqual(actual["boards_ordered"], 5)


if __name__ == "__main__":
    unittest.main()
