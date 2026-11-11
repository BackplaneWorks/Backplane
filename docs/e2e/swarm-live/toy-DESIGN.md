# Debounce design

Use a **saturating up/down integrator with hysteresis**, with default threshold
**M = 5**, for the encoder pin sampled every 0.5 ms.

Maintain a counter `c` in `[0, M]` and the debounced output `y`. Each input `1`
increments `c`, clamped at `M`; each `0` decrements it, clamped at zero. After
each update, set `y = 1` at `c == M`, set `y = 0` at `c == 0`, and otherwise
retain `y`. Emit one output per input sample. The separate switching thresholds
prevent chatter near an intermediate counter value.

An immediate-edge lockout would accept the first noise spike. Requiring M
consecutive equal samples with a shift register rejects glitches, but discards
all progress on one contrary sample. The integrator instead subtracts one count,
so a mostly settled edge can recover sooner. It needs only a counter and one
state bit, and treats rising and falling edges symmetrically.

## Why M = 5

The stated 2 ms bounce window is four sampling periods. Choosing
`floor(2.0 / 0.5) + 1 = 5` requires five net counts to cross from one saturated
state to the other. From a settled state, any isolated glitch of one through
four opposite samples is rejected; M = 4 would accept a four-sample glitch.
Contrary samples subtract evidence rather than resetting the counter.

A clean edge switches on its fifth new-level sample: 2.0 ms after the first
observed new sample, or approximately 2.0–2.5 ms after the physical edge,
depending on sample phase. After bouncing ends, five stable samples suffice
from any counter position, giving a conservative total response bound of about
4.5 ms from the physical edge when bounce lasts at most 2 ms.

This is a sampled-signal guarantee, not proof of rejecting every physical pulse
of duration at most 2 ms: five sample instants span exactly 2 ms, so endpoint
timing can expose five opposite samples. Repeated, biased noise can also
accumulate enough counts to switch. Valid encoder levels must persist long
enough to reach the opposite threshold; fast pulses can be suppressed. This
module cleans one pin, not quadrature direction or encoder position.

## Interface and verification

`debounce(samples, *, initial=None, threshold=5) -> list[int]` consumes any finite
iterable once and returns integer bits with the same length. By default, the
first sample seeds both state and counter; startup is accepted immediately.
Explicit `initial=0` or `1` starts at the corresponding saturated endpoint.
Empty input returns `[]`. Samples and explicit initial values accept integer
0/1 and booleans; invalid values raise `ValueError`. The threshold must be a
positive integer, excluding booleans. Each call starts fresh. Processing is
O(n), with O(1) filter state and O(n) returned storage.

Run `python3 -m pytest`. The 39 tests cover steady levels, exact clean-edge
latency, glitches in both polarities, bouncy edges, a seeded pulse train,
initialization, generators, booleans, custom thresholds, and invalid inputs.
