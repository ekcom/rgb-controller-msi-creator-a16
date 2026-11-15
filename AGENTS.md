# AGENTS.md — RGB Keyboard Controller

## Project Overview

Userspace USB HID driver and CLI for the **MSI Creator A16 AI+** laptop's per-key RGB keyboard controller (`VID 0x1462`, `PID 0x1565`). Reverse-engineered from USB packet captures of the official MSI Center software.

**Language:** Rust (edition 2021)
**Package name:** `rgb-controller-msi-creator-a16`

## Repository Structure

```
├── Cargo.toml                      # Package manifest
├── Cargo.lock                      # Pinned dependency versions
├── .gitignore                      # Ignores /target, /tmp
├── README.md                       # User-facing documentation (install, CLI usage, library)
│
├── src/
│   ├── main.rs                     # CLI entrypoint (clap parser, dry-run mode, USB dispatch)
│   ├── lib.rs                      # Library root — re-exports hid::{driver, protocol, transport}
│   │
│   ├── hid/                        # Core library module (USB HID implementation)
│   │   ├── mod.rs                  # Module declarations
│   │   ├── protocol.rs             # Protocol constants: USB IDs, timing delays, effect IDs,
│   │   │                           #   color stop templates, 98-key bitmask/serialization order
│   │   ├── driver.rs               # MsiKeyboardDriver<T: HidControlTransport> — high-level API
│   │   │                           #   (lighting effects, session lifecycle, per-key customize,
│   │   │                           #   music mode) + pure packet builder functions
│   │   └── transport.rs            # HidControlTransport trait + NusbTransport (real USB)
│   │                               #   + MockTransport (recording transport for tests/dry-run)
│   │
│   └── cli/                        # CLI layer (binary-only, not part of library)
│       ├── mod.rs                  # Module declarations
│       ├── args.rs                 # Clap argument definitions (Cli, CliColor, CliSpeed, etc.)
│       └── commands.rs             # Command dispatch (run_command), preset execution,
│                                   #   subcommand enum (Commands)
│
├── tests/
│   ├── pcap_conformance.rs         # Integration test suite — validates driver output byte-for-byte
│   │                               #   against captured USB traffic from official MSI software
│   └── support/
│       ├── capture.pcapng          # Wireshark USB capture (golden reference)
│       └── capture.csv             # Pre-exported tshark CSV (so CI doesn't need tshark)
│
├── docs/
│   ├── Protocol.md                 # Full reverse-engineered USB HID protocol specification
│   └── Capture-Guide.md           # Instructions for capturing new USB traces with Wireshark
│
├── .github/workflows/
│   └── ci.yml                      # GitHub Actions: lint (fmt + clippy), test, build + upload
│
└── tmp/                            # Development scratch files (gitignored)
    ├── scratch.md                  # Development notes / todo list
    ├── ref-impl.py                 # Python reference implementation (pyusb, VID 0x1462:0x1565)
    ├── script.py                   # Older Python script (different keyboard PID — DO NOT USE)
    └── analyze/                    # PDML capture analyzer script
```

## Architecture

### Dependency Graph

```
main.rs (binary)
  └── cli/args.rs, cli/commands.rs
        └── lib crate (rgb_controller_msi_creator_a16)
              ├── driver (MsiKeyboardDriver<T>)
              │     ├── protocol (constants, packet formats)
              │     └── transport (HidControlTransport trait)
              ├── protocol (re-exported)
              └── transport (re-exported)
                    ├── NusbTransport (real hardware via nusb crate)
                    └── MockTransport (in-memory recording for tests/dry-run)
```

### Key Design Patterns

- **Generic Transport Abstraction**: `MsiKeyboardDriver<T: HidControlTransport>` is parameterized over the transport, enabling hardware-free testing with `MockTransport` and a `--dry-run` CLI mode.
- **Lockstep Protocol Model**: Every `SET_REPORT` is paired with a `GET_REPORT` readback before the next command, enforced by the driver's `send_command_lockstep()`.
- **Pure Packet Builders**: All `build_*_packet()` functions are pure (no I/O), making them independently testable.
- **Protocol Validator Transport**: `ProtocolValidatorTransport` (in tests) is a state-machine that enforces all protocol invariants at the transport level — lockstep pairing, mandatory pre-clears, flash double-write, etc.

### Module Responsibilities

| Module | Responsibility |
|--------|---------------|
| `protocol.rs` | Constants only. USB IDs, timing delays, effect IDs, color palettes, key bitmasks. Zero logic. |
| `transport.rs` | `HidControlTransport` trait (set_idle, set_report, get_report, sleep). `NusbTransport` for real hardware. `MockTransport` for recording. |
| `driver.rs` | All protocol logic. Session lifecycle (init, profile switch, flash, discard). 12 lighting modes. Per-key customize. Music mode. Packet builders. |
| `args.rs` | CLI parsing. Color parsing (`#RRGGBB`, `R,G,B`, named colors). Speed/direction wrappers. |
| `commands.rs` | Maps CLI subcommands → driver calls. Preset system. `run_command()` dispatch. |

## Testing

### Test Suite Summary (18 tests)

**Library unit tests** (`src/hid/driver.rs` — 7 tests):
- Wire-format correctness for Breath, StarLight/Reactive disambiguation, Dance pre-clear, Music mode 65-byte readback, 98-key customize with union mask verification, flash apply double-send, per-key Ripple/Reactive headers.

**CLI unit tests** (`src/cli/` — 8 tests):
- Color parsing (valid/invalid), CLI argument parsing, speed/direction conversions, command display formatting, mock execution.

**Integration tests** (`tests/pcap_conformance.rs` — 3 tests):
- `test_entire_captured_csv_conforms_to_protocol_state_machine` — Replays all captured USB frames through `ProtocolValidatorTransport`.
- `test_driver_calls_match_captured_scenarios_byte_for_byte` — Compares driver output against captured wire traffic.
- `test_scenario_coverage_and_discrepancy_audit` — Ensures 100% scenario coverage (tested + explicitly excluded) with documented wire anomalies.

### Running Tests

```bash
cargo test                          # All tests (requires tests/support/capture.csv)
cargo test --test pcap_conformance  # Integration tests only
cargo test --lib                    # Library unit tests only
cargo test --bin rgb-controller-msi-creator-a16  # CLI tests only
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `nusb` 0.2 | Cross-platform USB device access (async, no C deps) |
| `clap` 4.6 (derive) | CLI argument parsing |
| `thiserror` 2.0 | Error type derivation |
| `csv` 1.3 (dev) | Parsing captured USB trace CSV |
| `serde` 1.0 (dev) | Deserializing CSV rows in integration tests |

## Key Conventions

- **All packet payloads are exactly 64 bytes**, zero-padded.
- **All multi-byte integers are little-endian** on the wire.
- **Effect IDs are shared** between autonomous and reactive modes, distinguished by `sub_mode_flag` (byte 9): `0x01` = autonomous (StarLight, Firework, Laser), `0x00` = reactive (Reactive, Ripple, Comet).
- **Dance mode requires a pre-effect global clear** (`02 02 06 00...`) before the dance packet.
- **Flash apply must be sent twice** with an inter-write delay (88ms global, 640ms customize).
- Protocol timing constants in `protocol.rs` are derived from empirical USB captures and must not be changed without re-capturing and re-validating.

## Common Tasks

### Adding a new lighting effect
1. Add protocol constants to `protocol.rs` (effect ID, period values, etc.)
2. Add a `build_*_packet()` function and `set_*()` driver method in `driver.rs`
3. Add unit test in `driver.rs` verifying wire format
4. Add CLI subcommand in `commands.rs` and wire it in `run_command()`
5. If captured reference exists, add test case in `pcap_conformance.rs`

### Updating the USB capture reference
Follow [`docs/Capture-Guide.md`](docs/Capture-Guide.md), then update `build_driver_test_cases()` and `EXCLUDED_SCENARIOS` in `tests/pcap_conformance.rs`.
