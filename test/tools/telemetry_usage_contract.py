"""Numeric client fixture contract, independent of receiver legacy constants."""

CLIENT_USAGE_COUNTS = (
    "threads_created", "user_messages", "assistant_messages", "bot_messages", "bot_wakes",
    "input_tokens", "cache_read_tokens", "cache_write_tokens", "output_tokens", "usage_turns",
    "boards_created", "boards_ordered",
)


def expected_counts(n):
    return dict(zip(CLIENT_USAGE_COUNTS, (n, n, n, n, n, 100*n, 20*n, 30*n, 40*n, n, n, n)))
