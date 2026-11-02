# Owner checklist

Hands-on checks only you can do (microphone, speaking to JARVIS, approving the
look). Tick each box and note anything odd next to it.

- [ ] **Task 3 — mic + speaker loopback.** Run `cargo run -p jarvis-core --example mic_loopback`
      (allow microphone access for your terminal if macOS asks). Expected: a 1 s 440 Hz tone is
      audible, and the printed `input rms` numbers (10 per second, for 5 s) rise clearly when you
      speak and fall back near 0 when quiet. Report the numbers you saw if it misbehaves.

## Phase 0
- [ ] **Window opens.** Run `cargo run -p jarvis-app`. Expected: an empty window titled "JARVIS" opens; close it. (Automated check: the binary stayed running 6 s without crashing.)
