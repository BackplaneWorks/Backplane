import Base
import ../src/core/usage-counts.bend as UC

law fixed_legacy_fields:
  {UC.Uc.names() == ["threads_created", "user_messages", "assistant_messages", "bot_messages", "bot_wakes", "input_tokens", "cache_read_tokens", "cache_write_tokens", "output_tokens", "usage_turns"] : List<&2, String>}

def fixed_legacy_fields():
  {==}

law fixed_pcb_fields:
  {UC.Uc.all_names() == ["threads_created", "user_messages", "assistant_messages", "bot_messages", "bot_wakes", "input_tokens", "cache_read_tokens", "cache_write_tokens", "output_tokens", "usage_turns", "boards_created", "boards_ordered"] : List<&2, String>}

def fixed_pcb_fields():
  {==}

def main() -> IO(Unit):
  IO.print("ok fixed legacy and PCB wire laws")
