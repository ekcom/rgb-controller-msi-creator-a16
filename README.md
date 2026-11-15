# RGB Keyboard Controller for MSI Creator A16 AI+

A userspace USB HID controller for the **MSI Creator A16 AI+** laptop's per-key RGB keyboard controller.

Contains a library and a CLI driver.

## Installation

### Supported Devices

- MSI Creator A16 AI+
    - Verified on BIOS release `E15FKAMS.321` (09/17/2026)
    - Other MSI `MS_1565` keyboards may share this controller but are not guaranteed
    - (USB VID `0x1462`, PID `0x1565`, Interface 0)

### Dependencies

- rust

### Configure udev Rule

To interact with the keyboard controller without root privileges, you must install a udev rule.

Create `/etc/udev/rules.d/60-rgb-controller-msi-creator-a16.rules`:

```
SUBSYSTEMS=="usb|hidraw", ATTRS{idVendor}=="1462", ATTRS{idProduct}=="1565", TAG+="uaccess"
```

To apply without rebooting:

```bash
sudo udevadm control --reload-rules && sudo udevadm trigger
```

## CLI

Run with `--help` to inspect all subcommands and parameters:
```bash
cargo run -- --help
```

### Global Options

- `--dry-run`: Runs the command against an in-memory mock transport and prints every generated USB HID Feature Report without touching hardware

```bash
cargo run -- --dry-run steady blue
```

### Core Session Commands

- **`init`**: Execute standard session startup handshake sequence.
    ```bash
    cargo run -- init
    ```

- **`off`**: Turn off all keyboard RGB LEDs.
    ```bash
    cargo run -- off
    ```

- **`apply [--customize]`**: Persist current volatile preview lighting to onboard non-volatile flash memory. Pass `--customize` when saving 98-key per-key layouts (to allow for controller flash timing).
    ```bash
    cargo run -- apply
    cargo run -- apply --customize
    ```

- **`discard`**: Discard unsaved preview changes and reset keyboard lighting.
    ```bash
    cargo run -- discard
    ```

### Lighting Effects

*All color options support hex codes (`#ff0000`, `ff0000`, `0x00ff00`), RGB decimal components (`255, 185, 0`), and color names (`red`, `green`, `blue`, etc).*

#### Full-Keyboard

- **`steady`**: Static color
    ```bash
    cargo run -- steady red
    cargo run -- steady '#00ffcc'
    cargo run -- steady '255, 185, 0'
    ```

- **`breath`**: Pulses flashes, from off, of alternating foreground and background colors
    ```bash
    cargo run -- breath --fg cyan --bg white --speed slow
    cargo run -- breath --fg red --bg yellow --speed fast
    ```

- **`color-cycle`**: Fades between colors in a loop
    ```bash
    cargo run -- color-cycle --speed medium
    cargo run -- color-cycle --speed fast --colors pink yellow cyan
    ```

- **`wave`**: Sweeps colors horizontally
    ```bash
    cargo run -- wave --direction left-to-right --speed medium
    cargo run -- wave --direction right-to-left --speed fast
    cargo run -- wave --direction ltr --speed slow --colors red white blue
    ```

- **`dance`**: Erratic color sequence using predetermined colors, independently looping on each key
    ```bash
    cargo run -- dance --speed fast
    ```

#### Across-Keys

* **`starlight`**: Twinkling effect where random keys illuminate and fade back off
    ```bash
    cargo run -- starlight --speed fast
    cargo run -- starlight --color orange --speed medium
    ```

* **`firework`**: Radial bursts of color exploding outward from random keys
    ```bash
    cargo run -- firework --speed fast
    cargo run -- firework --color cyan --speed slow
    ```

* **`laser`**: Horizontal, directional lines of color streak across the keys randomly
    ```bash
    cargo run -- laser --speed fast
    ```

#### Keypress-Reactive

* **`reactive`**: On key press, the key is illuminated and fades back to dark
    ```bash
    cargo run -- reactive --color white --speed fast
    cargo run -- reactive --color red --speed medium
    ```

* **`ripple`**: On key press, a radial burst of color explodes outward from the key
    ```bash
    cargo run -- ripple --speed fast
    cargo run -- ripple --color cyan --speed medium
    ```

* **`comet`**: On key press, a horizontal line of color streaks across the keys in either direction
    ```bash
    cargo run -- comet --color orange --speed fast
    ```

### Per-Key Customize Mode (`customize`)

Controls individual key LED assignments across the 98 addressable LEDs:
```bash
# Esc key solid white, remaining 97 keys running fast dance
cargo run -- customize --preset esc-white-rest-dance

# All 98 keys running fast dance
cargo run -- customize --preset all-dance

# All 98 keys solid red
cargo run -- customize --preset all-steady-red
```

### Demo Presets (`preset`)

Pre-configured presets:
```bash
# View list of available presets
cargo run -- preset --help

# Run specific preset
cargo run -- preset breath-cyan-white-slow
cargo run -- preset steady-red
cargo run -- preset wave-rainbow-ltr
cargo run -- preset starlight-rainbow-fast
cargo run -- preset reactive-white-fast
cargo run -- preset dance-fast
cargo run -- preset customize-esc-white-rest-dance
```

## Library

Generate documentation:
```
cargo doc
```

## Development

### Protocol

The reverse-engineered protocol is documented in [`docs/Protocol.md`](docs/Protocol.md).

### Tests

```bash
cargo test
```
