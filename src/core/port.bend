import Base

# Shell that runs on Linux and macOS alike. macOS has no timeout(1) or
# setsid(1) (coreutils and util-linux are GNU's); Perl's POSIX functions
# let the fallback supervise a command and its descendants together.

# Keep the supervisor outside the child's process group. Killing only the
# command leaves its children holding Proc.run's output pipes open. Complete
# the KILL grace period even if the immediate child already accepted TERM.
def Port.perl() -> String:
  "use POSIX qw(setpgid); my $t=shift; my $p=fork; defined($p) or exit 125; " ++
  "if (!$p) { setpgid(0,0)==0 or exit 125; exec {$ARGV[0]} @ARGV; exit 127; } " ++
  "setpgid($p,$p); " ++
  "my $stop=sub { my ($sig,$code)=@_; kill $sig,-$p; sleep 1; kill \"KILL\",-$p; waitpid($p,0); exit $code; }; " ++
  "$SIG{ALRM}=sub { $stop->(\"TERM\",124); }; " ++
  "$SIG{HUP}=sub { $stop->(\"HUP\",129); }; $SIG{INT}=sub { $stop->(\"INT\",130); }; " ++
  "$SIG{QUIT}=sub { $stop->(\"QUIT\",131); }; $SIG{TERM}=sub { $stop->(\"TERM\",143); }; " ++
  "alarm $t; my $r; do { $r=waitpid($p,0); } while ($r<0 && $!{EINTR}); " ++
  "my $s=$?; alarm 0; exit($r<0 ? 125 : ($s & 127) ? 128+($s & 127) : $s >> 8);"

# `tmo SECS CMD ARGS...`: CMD, stopped after SECS (killed a second later)
def Port.tmo() -> String:
  "tmo() { t=$1; shift; if command -v timeout >/dev/null 2>&1; then timeout -k 1 \"$t\" \"$@\"; elif command -v gtimeout >/dev/null 2>&1; then gtimeout -k 1 \"$t\" \"$@\"; else perl -e '" ++ Port.perl() ++ "' \"$t\" \"$@\"; fi; }; "

# sh's arguments that run timeout(1)'s (SECS CMD ARGS...):
# P.Proc.run("sh", Port.timeout(["5", "tailscale", ...]))
def Port.timeout(+args: List<&2, String>) -> List<&2, String>:
  "-c" <> (Port.tmo() ++ "tmo \"$@\"") <> "sh" <> args
