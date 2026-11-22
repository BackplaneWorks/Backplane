#!/bin/sh
# The nextest filter for the hub's tests on Windows: everything except the tests that do not hold there yet (docs/e2e/MATRIX.md, Windows).
# CI (hub windows-latest) passes this to `cargo nextest run -E`. To un-skip a test, delete it here.
set -eu
# the hub's tests that do not hold on Windows, each group with why (docs/e2e/MATRIX.md, Windows); "debt" is to be ported, the rest is Unix by design
skip='test(=agents::tests::sbs_child_probe)'
# POSIX processes: the stand-ins report Git sh's own pids and signals, which no Windows call reaches
skip="$skip | test(=agents::proc::tests::terminate_kills_the_whole_group_and_leaves_no_zombie) | test(=agents::tests::a_killed_process_is_named_as_probably_out_of_memory) | test(=agents::tests::interrupting_a_codex_turn_ends_its_process_and_keeps_the_ride) | test(=agents::tests::shutdown_leaves_no_process_behind)"
# POSIX files: exec bits, symlinks and hard links refused by O_NOFOLLOW, mode bits, names Windows cannot hold
skip="$skip | test(=agents::proc::tests::test_mode_runs_only_the_standin_folder) | test(=bots::tests::mail_tests::mail_recover_gives_a_journaled_body_its_header_once) | test(=bots::tests::store_tests::the_journal_is_not_opened_through_a_link) | test(=files::attach::tests::receives_whole_files_and_refuses_short_big_and_linked_ones) | test(=files::dir::tests::lists_folders_then_files_hides_dotfiles_and_symlinked_folders) | test(=files::rv::tests::a_snapshot_with_a_link_or_an_escaping_sheet_is_refused) | test(=files::serve::tests::serves_ranges_and_refuses_non_files_and_links) | test(=settings::accounts_io::tests::the_registry_adds_renames_removes_and_survives_a_restart_and_a_bad_file_is_not_trusted) | test(=update::run::tests::a_hard_link_found_after_unpacking_is_refused)"
# Windows debt, not ported: the runner's real gh is found ahead of the test's fake; the copy-outs parent (its probe passes alone)
skip="$skip | test(=files::git::tests::stacked_actions_against_a_local_bare_remote) | test(=net::copy_outs::no_alert_driven_out_leaves_on_a_copy_and_the_same_probe_posts_without_the_flag)"
# Windows by design: the agy runner test reads the file while 15 other threads rename onto it, and Windows refuses an open in the instant of a replace (the hub leaves a whole runner alone)
skip="$skip | test(=agents::providers::agy::tests::the_runner_is_written_whole_by_turns_starting_together)"
# Unix by design: a killed hub's child and grandchild are Git sh's pids, which no Windows call reaches (Windows has its own: plat::windows::f23_tests)
skip="$skip | test(=agents::tests::a_killed_hub_leaves_no_agent_behind)"
# Unix by design: the setup-token stand-in is a Python program on a pty (termios, tty.setraw), which Windows' Python has no module for; Windows' ConPTY sign-in is the real CLI's
skip="$skip | test(=settings::accounts_io::tests::a_sync_codes_answer_always_ends_the_attempt_refused_unanswered_or_cancelled_and_it_can_be_made_again) | test(=settings::accounts_io::tests::a_sync_token_is_made_by_setup_token_kept_in_the_vault_used_by_its_turns_and_carried_to_another_machine)"
# Windows by design: the loopback-only pairing test names its always-on hub http://0.0.0.0:<door>, a non-loopback address Linux and macOS connect
# to on this computer and Windows does not (WSAEADDRNOTAVAIL); a door on the runner's own address pairs, but the far calls after it need more rework
skip="$skip | test(=net::alwayson::tests::a_loopback_only_hub_links_to_a_public_door_and_is_reached_over_its_own_poll)"
echo "not ($skip)"
