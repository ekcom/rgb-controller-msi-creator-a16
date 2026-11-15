# MSI Creator A16 AI+ Per-Key RGB Keyboard Controller Programming Guide

## 0. Notation

- `i..j` represents the inclusive range from `i` to `j` (all integers `x` such that `i <= x <= j`)
- `ab` (in `code fence`) denotes the hexadecimal byte `0xab` (not base-10)
- `ab cd` denotes the byte `0xab` followed by `0xcd`
- `<ab * n>` denotes $n$ consecutive bytes of `0xab`
    - E.g., `<2A * 3>` expands to `2A 2A 2A`

## 1. USB HID Transport Layer

### 1.1 Setup Stage Parameters

To control the **MSI Creator A16 AI+ Per-Key RGB Keyboard Controller**, open the USB HID interface matching the following hardware descriptors:

| Parameter Name | Field | Value | Notes |
| :--- | :--- | :--- | :--- |
| **Vendor ID (VID)** | `usb.idVendor` | `0x1462` | Micro Star International (MSI) |
| **Product ID (PID)** | `usb.idProduct` | `0x1565` | Per-Key RGB Keyboard Controller |
| **Firmware Revision** | `usb.bcdDevice` | `0x0800` | Release 8.00 |
| **USB Specification** | `usb.bcdUSB` | `0x0200` | USB 2.0 (`bMaxPacketSize0` = 64 bytes) |
| **USB Interface** | `usb.bInterfaceClass` | `0x03` (HID) | Interface 0, Alternate Setting 0, Protocol `0x00` |
| **HID Descriptor** | `bcdHID` | `0x0111` | HID v1.11 (`wDescriptorLength` = 44 bytes) |
| **Control Endpoint** | `usb.endpoint_address` | `0x00` (OUT) / `0x80` (IN) | Carries all `SET_IDLE`, `SET_REPORT`, and `GET_REPORT` traffic |
| **Interrupt Endpoints** | `usb.bEndpointAddress` | `0x81` (IN) / `0x01` (OUT) | 64-byte max packet size, 1 ms interval (`bInterval = 1`); unused |
| **Feature Report Size** | `wLength` | 64 bytes (`0x0040`) | All host-to-device commands are fixed-length 64-byte HID Feature Reports (`ReportType = 0x03`) |

### 1.2 Core Transport Rules

1. **Communication Channel**: All communication with the controller takes place over **Control Endpoint 0** (`0x00` Host-to-Device, `0x80` Device-to-Host) using USB HID Class-Specific Interface requests (`URB_FUNCTION_CLASS_INTERFACE`, `0x001b`). Interrupt endpoints `0x81` and `0x01` are unused.

2. **`SET_REPORT` + `GET_REPORT` Lockstep Pairing**: Every single `SET_REPORT` command sent to the controller **must** be paired in strict lockstep with an immediately following `GET_REPORT` request on Endpoint `0x80` before the next `SET_REPORT` command is issued.
    - **Command-to-Readback Microsecond Window**: The host issues `GET_REPORT` within **22–440 µs** after receiving the `SET_REPORT` completion status stage, with a turnaround time of **3.46-4.35 ms**
      - **Standard Commands**: Issued within **22–299 µs** (median 64 µs) after `SET_REPORT` completion; readback completes in **3.49–4.35 ms** (median 3.94 ms).
      - **Music Mode Commands**: Issued within **35–326 µs** (median 64.5 µs) after `SET_REPORT` completion; readback completes in **3.66–3.97 ms** (median 3.93 ms).
      - **Flash Apply Commands**: Issued within **95–440 µs** (median 166.5 µs) after `SET_REPORT` completion; readback completes in **3.46–3.94 ms** (median 3.81 ms).
3. **Inter-Packet Key-to-Effect Delay (`02 01` + `02 02`)**: When applying lighting updates via paired key-selection mask (`02 01`) and effect-assignment (`02 02`) packets, the host waits an inter-packet delay of **36–368 µs** after `02 01` `GET_REPORT` readback completion before transmitting the `02 02` `SET_REPORT`.

4. **Synchronous USB Control Transfer Timing**: Configure the USB control transfer timeout to **at least 500–1000 ms**. Hardware turnaround for `SET_REPORT` is bifurcated depending on the command category:
    - **Standard Non-Flash Commands**: Complete in **2.12–12.97 ms** (ranging from 2.12 ms for simple handshakes up to 12.97 ms for full global effect updates; see §2.1 for exact per-command measurements).
    - **Flash Write Commands (`02 a0`)**: Blocks in hardware inside the `SET_REPORT` transfer stage for **255.15–256.00 ms** while erasing and committing state to internal non-volatile flash memory. (A synchronous USB control transfer with an adequate timeout automatically blocks until the flash write completes without requiring artificial sleep timers between `SET_REPORT` and `GET_REPORT`.)

5. **Zero-Padding**: Every `SET_REPORT` data payload is exactly 64 bytes (`usb.data_fragment = 64` bytes; total USB setup frame `usb.data_len = 72` bytes including the 8-byte Setup packet). Always zero-pad unused trailing bytes (`00`) through Byte 63.
> [!NOTE]
> **Stock Software Residual Bytes**:
> The official MSI software occasionally leaves stale residual bytes from a previous effect in unused fields (Bytes 3..38) when switching to Off or interactive Dance.
> However, driver implementations are recommended to zero-pad all unused bytes.

6. **Little-Endian Encoding**: All multi-byte integer fields (such as `period_ms` and USB setup fields `wValue`, `wIndex`, and `wLength`) are **little-endian** on the wire.

### 1.3 Transport Request Types

#### 1.3.1 Bus Silence (`SET_IDLE`, `bRequest = 0x0a`)

Silences periodic Interrupt IN reports unless state changes occur.

Send once at session initialization (and optionally prior to interactive effect changes).

- **Setup Data**: `21 0a 00 00 00 00 00 00`
    - `bmRequestType`: `0x21` (Host-to-Device, Class, Interface)
    - `bRequest`: `0x0a` (`SET_IDLE`)
    - `wValue`: `0x0000` (Report ID = 0 [all reports], Duration = 0 [indefinite])
    - `wIndex`: `0x0000` (Interface 0)
    - `wLength`: `0x0000` (0 bytes; no data payload)
- **Response**: 0-byte status stage (`USBD_STATUS_SUCCESS`), completing in **1.19–2.17 ms** (median 1.69 ms) turnaround

#### 1.3.2 Sending a Command (`SET_REPORT`, `bRequest = 0x09`)

Transmits a 64-byte HID Feature Report (`ReportType = 0x03`) to the controller.

- **Setup Data**: `21 09 <ReportID> 03 00 00 40 00`
    - `bmRequestType`: `0x21` (Host-to-Device, Class, Interface)
    - `bRequest`: `0x09` (`SET_REPORT`)
    - `wValue`:
        - `0x0301` for Feature Report ID 1 (Handshake & State Query)
        - `0x0302` for Feature Report ID 2 (Key Masks, Lighting Effects, Music, Flash Save/Discard)
    - `wIndex`: `0x0000` (Interface 0)
    - `wLength`: `0x0040` (64 bytes)
    - **Data Payload (64 bytes)**: Command-specific; see §2.1
- **Response**: 0-byte status stage (`USBD_STATUS_SUCCESS`). Device turnaround is **2.12–12.97 ms** for standard commands and **255.15–256.00 ms** for flash write commands (see §2.1 for exact per-command turnaround times).

#### 1.3.3 Reading Command Status (`GET_REPORT`, `bRequest = 0x01`)

Two variants exist depending on the preceding `SET_REPORT` command:

- **Standard Readback** (after handshake, standard, lighting, flash, or discard commands):
    - **Setup Data**: `a1 01 01 03 00 00 40 00`
        - `bmRequestType`: `0xa1` (Device-to-Host, Class, Interface)
        - `bRequest`: `0x01` (`GET_REPORT`)
        - `wValue`: `0x0301` (Feature Report ID 1, wire bytes `01 03`)
        - `wIndex`: `0x0000` (Interface 0)
        - `wLength`: `0x0040` (64 bytes)
    - **Timing**: Issued within **22–299 µs** (median 64 µs) after standard `SET_REPORT` completion; completes in **3.46–4.35 ms**
   - **Response**: 64 bytes
        - Response contents echo the controller state (previous `SET_REPORT` payload)

- **Music Mode Stream Readback** (Follows `02 05` and `02 06` only):
   - **Setup Data**: `a1 01 02 03 00 00 41 00`
     - `bmRequestType`: `0xa1` (Device-to-Host, Class, Interface)
     - `bRequest`: `0x01` (`GET_REPORT`)
     - `wValue`: `0x0302` (Feature Report ID 2, wire bytes `02 03`)
     - `wIndex`: `0x0000` (Interface 0)
     - `wLength`: `0x0041` (*65* bytes)
   - **Timing**: Issued within **35–326 µs** (median 64.5 µs) after `SET_REPORT` completion; completes in **3.66–3.97 ms**
   - **Response**: *65* bytes
        - Response contents echo the streaming audio frame status (previous `SET_REPORT` payload)

## 2. Command Overview

### 2.1 Command Options

Every 64-byte Feature Report (SET\_REPORT) command begins with a 2-byte prefix (Byte 0 = Report ID, Byte 1 = Subcommand Opcode), followed by command-specific parameters in Bytes 2..63:

| Command Purpose | wValue | Bytes 0..1 | Bytes 2..63 Payload Summary | SET Timing Delay | GET Turnaround | Packet Notes |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Device Handshake** | `0x0301` | `01 91` | Byte 2 = `01`; Bytes 3..63 = `00` | 2.21–3.12 ms | 3.79–4.00 ms | - |
| **State/Profile Query** | `0x0301` | `01 a0` | Bytes 2..63 = `00` | 2.15–3.05 ms | 3.88–4.03 ms | - |
| **Select Key Bitmask** | `0x0302` | `02 01` | Bytes 2..33 = 32B Key Mask; Bytes 34..63 = `00` | 2.12–3.75 ms | 3.49–4.35 ms | 36–368 µs gap before paired `02 02` |
| **Set Effect** | `0x0302` | `02 02` | Bytes 2..10 = Header; Bytes 11..38 = Stops; Bytes 39..63 = `00` | Global: 7.50–12.97 ms<br>Per-Key: 3.48–4.41 ms | 3.80–4.09 ms | - |
| **Music Init Palette** | `0x0302` | `02 05` | Bytes 2..4 = `00`; Bytes 5..19 = 5 RGB stops; Byte 20 = `ff`; Bytes 21..63 = `00` | 2.73 ms | 3.92 ms (65B) | GET\_REPORT `wLength = 65` |
| **Music Audio Frame** | `0x0302` | `02 06` | Bytes 2..63 = 62B real-time audio visualization data | 2.13–3.14 ms | 3.66–3.97 ms (65B) | GET\_REPORT `wLength = 65`. Streamed every 14.64–17.76 ms (~65.5 Hz) |
| **Save to Flash (Apply)** | `0x0302` | `02 a0` | Bytes 2..63 = `00` | **255.15–256.00 ms** | 3.46–3.94 ms | **Must be sent twice**, with a gap of 85–90 ms (global) / ~640 ms (per-key) |
| **Discard Changes** | `0x0302` | `02 b0` | Bytes 2..63 = `00` | 9.34–10.12 ms | 3.89–3.93 ms | - |

### 2.2 Standard Lifecycle Workflows

Recall: All "Send `X`" messages are to be zero-padded up to 64 bytes

Note that these are based on the empirical behavior from the stock app capture.
They are not necessarily completely logical but should be followed to reduce risk of corruption/bricking.

#### 2.2.1 Application Startup / Session Initialization

Execute this sequence when opening a connection to the keyboard controller:

1. **Bus Silence**: *(Optional on re-open)* Send `SET_IDLE` (`21 0a 00 00 00 00 00 00`)
2. **Handshake #1**: Send `01 91 01` + `GET_REPORT(0x0301)`; wait **1.8–2.5 ms**
3. **State Query #1**: Send `01 a0 00` + `GET_REPORT(0x0301)`; wait **118–120 ms**
4. **Handshake #2**: Send `01 91 01` + `GET_REPORT(0x0301)`; wait **273–287 ms**
5. **State Query #2**: Send `01 a0 00` + `GET_REPORT(0x0301)`; wait **3.3–3.9 ms**
6. **Handshake #3**: Send `01 91 01` + `GET_REPORT(0x0301)`; wait **139–153 ms**
7. **Apply Lighting State *Twice***: Transmit the initial lighting state pair (`02 01 <mask>` + `GET_REPORT(0x0301)` & `02 02 <config>` + `GET_REPORT(0x0301)`) **twice** in succession, separated by a **43–68 ms** host delay

#### 2.2.2 Switching Profiles

Execute this sequence when switching between onboard lighting profiles:

1. **Discard Unsaved Changes On Current Profile** *(if desired)*: Send `02 b0` *(+ 62 zero bytes)* + `GET_REPORT(0x0301)`
2. **Delay** on the host for **542.59–549.72 ms**
3. **Profile Query #1**:
    1. Send `01 a0 00` + `GET_REPORT(0x0301)`
    2. Wait **5.78–9.55 ms**
    3. Send `01 91 01` + `GET_REPORT(0x0301)`
4. **Delay** on the host for **52.96–67.94 ms**
5. **Profile Query #2**:
    1. Send `01 a0 00` + `GET_REPORT(0x0301)`
    1. Wait **4.89–9.27 ms**
    1. Send `01 91 01` + `GET_REPORT(0x0301)`
6. **Delay** on the host for **20.20–43.92 ms**
7. **Apply Target Profile Lighting**:
    - **For Standard Effects / Mode Off**: Send lighting configuration (`02 01 <mask>` + `02 02 <config>`) **twice** in succession, separated by an inter-cycle delay of **42.88 ms**
    - **For Dance Mode (0x00) Profiles**: Transmit **four** (`02 01` + `02 02`) pairs in succession (**two full clear-and-set cycles**):
        1. *Cycle 1*: Send Pre-Effect Global Clear (see §3.2), wait **0.76–1.39 ms**, send Dance mode (`02 01 <ff * 32>` + `02 02 00 64 00 32 32 0a 01 00 00...`)
        2. *Delay*: Wait **13.23–30.28 ms**
        3. *Cycle 2*: Send Pre-Effect Global Clear (see §3.2), wait **0.76–1.39 ms**, send Dance mode (`02 01 <ff * 32>` + `02 02 00 64 00 32 32 0a 01 00 00...`)

#### 2.2.3 Saving Active Lighting to Non-Volatile Flash

> This matches the behavior of pressing the "Apply" button.

Changes sent via Set Effect packets (`02 01` + `02 02`) take effect immediately.
But, these changes are only saved in volatile preview and revert if discarded or device shut down. To persist the active state to onboard flash memory:

1. **Save to Flash #1**: Send `02 a0` + GET\_REPORT(0x0301). **This will block for ~256 ms.**
2. **Delay** on the host:
   - Wait **85–90 ms** if saving a **Global Effect**.
   - Wait **~640 ms** if saving a **Per-Key (Customize) Layout**.
3. **Save to Flash #2**: Send `02 a0` + GET\_REPORT(0x0301) again.

#### 2.2.4 Discarding Preview Changes & Closing the Session

> This matches the behavior of pressing the "Discard" button.

To revert unsaved preview changes and shut down the control session cleanly:

1. **Discard Changes**: Send `02 b0` + GET\_REPORT(0x0301)
2. **Delay**: Wait **~55 ms**
3. **Send Global Off Sequence**: mask `02 01 <ff * 32>` + GET\_REPORT(0x0301) & off `02 02 06` + GET\_REPORT(0x0301)

## 3. Effect Configuration

### 3.1 Command Sequence

Updating keyboard lighting always requires **two consecutive `SET_REPORT(0x0302)` commands** (each paired in strict lockstep with an immediately following `GET_REPORT(0x0301)`):

1. **Key Selection (`02 01`)**: Defines a 256-bit (32-byte) bitmask specifying which physical keys will receive the effect.

2. **Inter-Packet Key-to-Effect Delay**: The host waits **36–368 µs** after the `02 01` `GET_REPORT` readback completes before transmitting the `02 02` command.

3. **Effect Assignment (`02 02`)**: Assigns the animation mode, period, coordinates, density, speed step, and RGB color keyframes to the selected keys.

### 3.2 Pre-Effect Global Clear

Before switching into **Dance Mode** (`02 02 00`), **Music Mode** (`02 05`/`02 06`), or writing a new full **Per-Key (Customize)** layout, the controller state must first be cleared using the all-zero Off packet:

1. **Select All Keys**: Send `02 01 <ff * 32>` + `GET_REPORT(0x0301)`
2. **All-Zero Off Clear**: Send `02 02 06 <00 * 61>` + `GET_REPORT(0x0301)`

After the clear sequence completes, pause before transmitting the destination mode commands:
- **Dance Mode**: Wait **0.76–1.39 ms** (both for interactive mode changes and profile switches)
- **Customize Mode Sweep**: Wait **2.18–2.41 ms** before transmitting the 98 per-key pairs
- **Music Mode**: Wait **~146 ms** before sending the `02 05` palette initialization packet

### 3.3 Effect Assignment Packet

| Byte(s) | Field | Value |
| :--- | :--- | :--- |
| **0..1** | Prefix | Always `02 02` for effect assignment |
| **2** | Effect ID | Effect-specific; see §3.4 |
| **3..4** | Period (ms) | Animation cycle duration, formatted as a uint16 in little-endian. Smaller = faster; see §4 for exact per-mode values. *(Uncertainty note: Steady and Off use `9c ff` (65436 unsigned / -100 signed 16-bit); MCU interpretation of this field is uncertain.)* |
| **5** | Direction x | Horizontal coordinate (0..100, where 50 = center). Wave: `64` = Left-to-Right, `00` = Right-to-Left; Steady, Breath, Color Cycle, Off, Profile-Switch Dance, Per-Key Customize: `32` = Neutral/Center; StarLight, Firework, Laser, Reactive, Ripple, Comet, interactive Dance: `00` |
| **6** | Direction y | Vertical coordinate / brightness scale. All-zero pre-effect global off: `00`; all other effects: `32` |
| **7** | Density Param | Effect spread / tail density. Steady, Breath (global), Color Cycle, Wave, Dance (global), StarLight, Ripple, Off: `0a`; Firework: `1e`; Laser, Reactive (global), Comet, Per-Key Breath/Dance/Reactive: `50`; All-zero pre-effect global off: `00` |
| **8** | Speed/Step | Secondary speed control / step increment. **StarLight**: Fast = 0x01, Med = 0x03, Slow = 0x05; **Firework**: Fast = 0x07, Med = 0x0a, Slow = 0x0d; **Laser**: Fast = 0x02, Med = 0x04, Slow = 0x06; **Reactive / Ripple / Comet (Global)**: 0x02; **Steady / Breath / Color Cycle / Wave / Dance / Standard Off (Global)**: 0x01 |
| **9** | Effect Sub-ID | Selects between paired effects sharing effect\_id: 0x01 = **Autonomous continuous animation** (StarLight, Firework, Laser, Per-Key Wave, Sweep 1 Per-Key Dance); 0x00 = **Key-press reactive animation** (Reactive, Ripple, Comet); All other global modes: 0x00 |
| **10** | Reserved | Always `00` |
| **11..38** | Color Stops | Up to 7 keyframes of 4 bytes each: \[R, G, B, Position\], where $R,G,B \in [0,255]$ and $Position \in [0,100]$ (in hex). Unused stops are 0x00. See §3.5 |
| **39..63** | Padding | Always `00` |

### 3.4 Effects

| Name | Effect ID (Byte 2) | Effect Sub-ID (Byte 9) | Description |
| :--- | :--- | :--- | :--- |
| Dance | `00` | (`00`) | Erratic color sequence using predetermined colors, independently looping on each key |
| Steady | `01` | (`00`) | Solid color |
| Breath | `02` | (`00`) | Pulses flashes, from off, of alternating foreground and background colors |
| Color Cycle | `03` | (`00`) | Fades between colors in a loop |
| Wave | `04` | (`00`) | Sweeps colors horizontally |
| StarLight | `05` | `01` | Twinkling effect where random keys illuminate and fade back off |
| Reactive | `05` | `00` | **Reactive.** On key press, the key is illuminated and fades back to dark |
| Off | `06` | (`00`) | Turns LEDs off |
| Firework | `07` | `01` | Radial bursts of color exploding outward from random keys |
| Ripple | `07` | `00` | **Reactive.** On key press, a radial burst of color explodes outward from the key |
| Laser | `08` | `01` | Horizontal, directional lines of color streak across the keys randomly |
| Comet | `08` | `00` | **Reactive.** On key press, a horizontal line of color streaks across the keys in either direction |

### 3.5 Color Stop Templates

Every effect with user-configurable colors uses one of the color keyframe templates detailed below in Bytes 11..38 (with all remaining bytes through Byte 63 set to 0x00).

Each color stop has the format `<red> <green> <blue> <keyframe position>`; red/green/blue has a range of 0-255 (encoded as hex), and keyframe position has a range of 0-100 (encoded as hex).

#### 3.5.1 Steady: Single-Color Loop

Encodes a solid color at 100% (0x64) followed by an identical wrap-around stop at 0% (0x00):

1. Bytes 11..14: `<R> <G> <B> 64`
2. Bytes 15..18: `<R> <G> <B> 00`

*Example:*
White (255, 255, 255): `ff ff ff 64 ff ff ff 00`

#### 3.5.2 Breath: Two-Color Foreground + Background

Encodes the foreground color at 50%, the background color at 100%, and a wrap-around stop returning to the foreground color at 0% (0x00):

1. Bytes 11..14: `FG_R FG_G FG_B 32`
2. Bytes 15..18: `BG_R BG_G BG_B 64`
3. Bytes 19..22: `FG_R FG_G FG_B 00`

*Example:*
FG White (255,255,255) + BG Orange (255,185,15): `ff ff ff 32 ff b9 0f 64 ff ff ff 00`

#### 3.5.3 Color Cycle & Wave: Color Loops

##### 3.5.3.1: Three-Color Loop

Places three user-selected colors at 33%, 66%, and 100%, followed by the first color repeated at 0%:

1. Bytes 11..14: `R1 G1 B1 21`
2. Bytes 15..18: `R2 G2 B2 42`
3. Bytes 19..22: `R3 G3 B3 64`
4. Bytes 23..26: `R1 G1 B1 00`

*Example:*
Pink (255,0,255), Yellow (255,255,0), Cyan (0,255,255): `ff 00 ff 21 ff ff 00 42 00 ff ff 64 ff 00 ff 00`

##### 3.5.3.2: Six-Color Rainbow Loop

Fixed 7-stop circular rainbow gradient with stops at 16%, 32%, 48%, 64%, 80%, 100%, and a wrap-around stop with the first color at 0%:

1. Bytes 11..14: `ff 00 00 10` (Red     @ 16%)
2. Bytes 15..18: `ff ff 00 20` (Yellow  @ 32%)
3. Bytes 19..22: `00 ff 00 30` (Green   @ 48%)
4. Bytes 23..26: `00 ff ff 40` (Cyan    @ 64%)
5. Bytes 27..30: `00 00 ff 50` (Blue    @ 80%)
6. Bytes 31..34: `ff 00 ff 64` (Magenta @ 100%)
7. Bytes 35..38: `ff 00 00 00` (Red     @ 0% (wrap))

*Full sequence:* `ff 00 00 10 ff ff 00 20 00 ff 00 30 00 ff ff 40 00 00 ff 50 ff 00 ff 64 ff 00 00 00`

#### 3.5.4 StarLight, Firework, Laser, Reactive, Comet, Ripple: Black Lead-In

Pop-up and key-reactive animations require a leading Black (`00 00 00`) keyframe instead of a trailing 0x00 wrap stop.

##### 3.5.4.1: Single Color, Black Lead-In

Places Black at 50% and the target color at 100%:

1. Bytes 11..14: `00 00 00 32`
2. Bytes 15..18: `R  G  B  64`

*Example:*
Cyan (128, 255, 255): `00 00 00 32 80 ff ff 64`

##### 3.5.4.2: Rainbow, Black Lead-In

Places Black at 14% followed by 6 rainbow stops spaced at 14% intervals up to 100%:

1. Bytes 11..14: `00 00 00 0e` (Black   @ 14%)
2. Bytes 15..18: `ff 00 00 1c` (Red     @ 28%)
3. Bytes 19..22: `ff ff 00 2a` (Yellow  @ 42%)
4. Bytes 23..26: `00 ff 00 38` (Green   @ 56%)
5. Bytes 27..30: `00 ff ff 46` (Cyan    @ 70%)
6. Bytes 31..34: `00 00 ff 54` (Blue    @ 84%)
7. Bytes 35..38: `ff 00 ff 64` (Magenta @ 100%)

*Full sequence:* `00 00 00 0e ff 00 00 1c ff ff 00 2a 00 ff 00 38 00 ff ff 46 00 00 ff 54 ff 00 ff 64`

## 4. Global Mode

### 4.1 Applying Effects in Global Mode

To apply any effect to the entire keyboard:

1. Send `02 01 <ff * 32>` (and Bytes 34..63 set to `00`), followed by `GET_REPORT(0x0301)`
2. Send `02 02` using the exact 11-byte header (Bytes 0..10) and Color Stop Template (Bytes 11..38) from the table below (padding Bytes 39..63 with 0x00), followed by `GET_REPORT(0x0301)`

### 4.2 Global Mode Effects Full List

| Mode | Speed / Direction Setting | Header Bytes 0..10 (Hex) | period\_ms (Dec) | Color Stops (Bytes 11..38) | Special Requirements |
| :---- | :---- | :---- | :---- | :---- | :---- |
| **Off** | Standard Global Off | 02 02 06 9c ff 32 32 0a 01 00 00 | 65436 (-100) | All 0x00 | Can also use period\_ms = 64 00 (100) |
| **Off** | Pre-Effect Clear | 02 02 06 00 00 00 00 00 00 00 00 | 0 | All 0x00 | Required before Dance, Music, or Customize sweeps (see §3.2) |
| **Steady** | Static (N/A) | 02 02 01 9c ff 32 32 0a 01 00 00 | 65436 (-100) | **Template 3.5.1** (\[R,G,B,64, R,G,B,00\]) | None |
| **Breath** | Slow | 02 02 02 b0 04 32 32 0a 01 00 00 | 1200 | **Template 3.5.2** (FG@32, BG@64, FG@00) | Speed is controlled by Bytes 3..4 (period\_ms) |
| **Breath** | Medium | 02 02 02 20 03 32 32 0a 01 00 00 | 800 | **Template 3.5.2** (FG@32, BG@64, FG@00) | Speed is controlled by Bytes 3..4 (period\_ms) |
| **Breath** | Fast | 02 02 02 90 01 32 32 0a 01 00 00 | 400 | **Template 3.5.2** (FG@32, BG@64, FG@00) | Speed is controlled by Bytes 3..4 (period\_ms) |
| **Color Cycle** | Slow | 02 02 03 b0 04 32 32 0a 01 00 00 | 1200 | **Template 3.5.3.2** (Rainbow) or **Template 3.5.3.1** (3-Color) | Speed is controlled by Bytes 3..4 (period\_ms) |
| **Color Cycle** | Medium | 02 02 03 20 03 32 32 0a 01 00 00 | 800 | **Template 3.5.3.2** (Rainbow) or **Template 3.5.3.1** (3-Color) | Speed is controlled by Bytes 3..4 (period\_ms) |
| **Color Cycle** | Fast | 02 02 03 90 01 32 32 0a 01 00 00 | 400 | **Template 3.5.3.2** (Rainbow) or **Template 3.5.3.1** (3-Color) | Speed is controlled by Bytes 3..4 (period\_ms) |
| **Wave** | Slow, Left $\rightarrow$ Right | 02 02 04 20 03 64 32 0a 01 00 00 | 800 | **Template 3.5.3.2** (Rainbow) or **Template 3.5.3.1** (3-Color) | Byte 5 (DirX) = 0x64 (100) for Left $\rightarrow$ Right |
| **Wave** | Med, Left $\rightarrow$ Right | 02 02 04 f4 01 64 32 0a 01 00 00 | 500 | **Template 3.5.3.2** (Rainbow) or **Template 3.5.3.1** (3-Color) | Byte 5 (DirX) = 0x64 (100) for Left $\rightarrow$ Right |
| **Wave** | Fast, Left $\rightarrow$ Right | 02 02 04 2c 01 64 32 0a 01 00 00 | 300 | **Template 3.5.3.2** (Rainbow) or **Template 3.5.3.1** (3-Color) | Byte 5 (DirX) = 0x64 (100) for Left $\rightarrow$ Right |
| **Wave** | Slow, Right $\rightarrow$ Left | 02 02 04 20 03 00 32 0a 01 00 00 | 800 | **Template 3.5.3.2** (Rainbow) or **Template 3.5.3.1** (3-Color) | Byte 5 (DirX) = 0x00 (0) for Right $\rightarrow$ Left |
| **Wave** | Med, Right $\rightarrow$ Left | 02 02 04 f4 01 00 32 0a 01 00 00 | 500 | **Template 3.5.3.2** (Rainbow) or **Template 3.5.3.1** (3-Color) | Byte 5 (DirX) = 0x00 (0) for Right $\rightarrow$ Left |
| **Wave** | Fast, Right $\rightarrow$ Left | 02 02 04 2c 01 00 32 0a 01 00 00 | 300 | **Template 3.5.3.2** (Rainbow) or **Template 3.5.3.1** (3-Color) | Byte 5 (DirX) = 0x00 (0) for Right $\rightarrow$ Left |
| **Dance** | Slow | 02 02 00 2c 01 00 32 0a 01 00 00 | 300 | All 0x00 (built-in pattern) | Precede with Pre-Effect Global Clear (see §3.2) |
| **Dance** | Medium | 02 02 00 c8 00 00 32 0a 01 00 00 | 200 | All 0x00 (built-in pattern) | Precede with Pre-Effect Global Clear (see §3.2) |
| **Dance** | Fast (1s) | 02 02 00 64 00 32 32 0a 01 00 00 | 100 | All 0x00 (built-in pattern) | Precede with Pre-Effect Global Clear (see §3.2; DirX can be 0x00 or 0x32) |
| **StarLight** | Slow | 02 02 05 2c 01 00 32 0a 05 01 00 | 300 (fixed) | **Template 3.5.4.2** (Rainbow) or **3.5.4.1** (Single) | Speed is controlled by Byte 8 (0x05); Byte 9 = 0x01 |
| **StarLight** | Medium | 02 02 05 2c 01 00 32 0a 03 01 00 | 300 (fixed) | **Template 3.5.4.2** (Rainbow) or **3.5.4.1** (Single) | Speed is controlled by Byte 8 (0x03); Byte 9 = 0x01 |
| **StarLight** | Fast | 02 02 05 2c 01 00 32 0a 01 01 00 | 300 (fixed) | **Template 3.5.4.2** (Rainbow) or **3.5.4.1** (Single) | Speed is controlled by Byte 8 (0x01); Byte 9 = 0x01 |
| **Firework** | Slow | 02 02 07 90 01 00 32 1e 0d 01 00 | 400 (fixed) | **Template 3.5.4.2** (Rainbow) or **3.5.4.1** (Single) | Speed is controlled by Byte 8 (0x0d); Byte 7 = 0x1e, Byte 9 = 0x01 |
| **Firework** | Medium | 02 02 07 90 01 00 32 1e 0a 01 00 | 400 (fixed) | **Template 3.5.4.2** (Rainbow) or **3.5.4.1** (Single) | Speed is controlled by Byte 8 (0x0a); Byte 7 = 0x1e, Byte 9 = 0x01 |
| **Firework** | Fast | 02 02 07 90 01 00 32 1e 07 01 00 | 400 (fixed) | **Template 3.5.4.2** (Rainbow) or **3.5.4.1** (Single) | Speed is controlled by Byte 8 (0x07); Byte 7 = 0x1e, Byte 9 = 0x01 |
| **Laser** | Slow | 02 02 08 82 00 00 32 50 06 01 00 | 130 (fixed) | **Template 3.5.4.2** (Rainbow) or **3.5.4.1** (Single) | Speed is controlled by Byte 8 (0x06); Byte 7 = 0x50, Byte 9 = 0x01 |
| **Laser** | Medium | 02 02 08 82 00 00 32 50 04 01 00 | 130 (fixed) | **Template 3.5.4.2** (Rainbow) or **3.5.4.1** (Single) | Speed is controlled by Byte 8 (0x04); Byte 7 = 0x50, Byte 9 = 0x01 |
| **Laser** | Fast | 02 02 08 82 00 00 32 50 02 01 00 | 130 (fixed) | **Template 3.5.4.2** (Rainbow) or **3.5.4.1** (Single) | Speed is controlled by Byte 8 (0x02); Byte 7 = 0x50, Byte 9 = 0x01 |
| **Reactive** | Slow | 02 02 05 f4 01 00 32 50 02 00 00 | 500 | **Template 3.5.4.1** (Single Color) or **3.5.4.2** | Speed is in Bytes 3..4; Byte 7 = 0x50, Byte 8 = 0x02, Byte 9 = 0x00 |
| **Reactive** | Medium | 02 02 05 2c 01 00 32 50 02 00 00 | 300 | **Template 3.5.4.1** (Single Color) or **3.5.4.2** | Speed is in Bytes 3..4; Byte 7 = 0x50, Byte 8 = 0x02, Byte 9 = 0x00 |
| **Reactive** | Fast | 02 02 05 64 00 00 32 50 02 00 00 | 100 | **Template 3.5.4.1** (Single Color) or **3.5.4.2** | Speed is in Bytes 3..4; Byte 7 = 0x50, Byte 8 = 0x02, Byte 9 = 0x00 |
| **Ripple** | Slow | 02 02 07 90 01 00 32 0a 02 00 00 | 400 | **Template 3.5.4.2** (Rainbow) or **3.5.4.1** (Single) | Speed is in Bytes 3..4; Byte 7 = 0x0a, Byte 8 = 0x02, Byte 9 = 0x00 |
| **Ripple** | Medium | 02 02 07 2c 01 00 32 0a 02 00 00 | 300 | **Template 3.5.4.2** (Rainbow) or **3.5.4.1** (Single) | Speed is in Bytes 3..4; Byte 7 = 0x0a, Byte 8 = 0x02, Byte 9 = 0x00 |
| **Ripple** | Fast | 02 02 07 c8 00 00 32 0a 02 00 00 | 200 | **Template 3.5.4.2** (Rainbow) or **3.5.4.1** (Single) | Speed is in Bytes 3..4; Byte 7 = 0x0a, Byte 8 = 0x02, Byte 9 = 0x00 |
| **Comet** | Slow | 02 02 08 e5 00 00 32 50 02 00 00 | 229 | **Template 3.5.4.2** (Rainbow) or **3.5.4.1** (Single) | Speed is in Bytes 3..4; Byte 7 = 0x50, Byte 8 = 0x02, Byte 9 = 0x00 |
| **Comet** | Medium | 02 02 08 aa 00 00 32 50 02 00 00 | 170 | **Template 3.5.4.2** (Rainbow) or **3.5.4.1** (Single) | Speed is in Bytes 3..4; Byte 7 = 0x50, Byte 8 = 0x02, Byte 9 = 0x00 |
| **Comet** | Fast | 02 02 08 50 00 00 32 50 02 00 00 | 80 | **Template 3.5.4.2** (Rainbow) or **3.5.4.1** (Single) | Speed is in Bytes 3..4; Byte 7 = 0x50, Byte 8 = 0x02, Byte 9 = 0x00 |

*Note: Some entries are inferred from a combination of observations, interpolated.*

### 4.3 Examples

#### 4.3.1 Steady — Solid Red

```
02 02 01 9c ff 32 32 0a 01 00 00 ff 00 00 64 ff
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
```

#### 4.3.2 Breath — Medium Speed, Foreground White, Background Orange

```
02 02 02 20 03 32 32 0a 01 00 00 ff ff ff 32 ff
b9 0f 64 ff ff ff 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
```

#### 4.3.3 Wave — Medium Speed, Left-to-Right, Rainbow

```
02 02 04 f4 01 64 32 0a 01 00 00 ff 00 00 10 ff
ff 00 20 00 ff 00 30 00 ff ff 40 00 00 ff 50 ff
00 ff 64 ff 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
```

#### 4.3.4 StarLight — Fast Speed, Rainbow

```
02 02 05 2c 01 00 32 0a 01 01 00 00 00 00 0e ff
00 00 1c ff ff 00 2a 00 ff 00 38 00 ff ff 46 00
00 ff 54 ff 00 ff 64 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
```

#### 4.3.5 Reactive — Fast Speed, Single Color White

```
02 02 05 64 00 00 32 50 02 00 00 00 00 00 32 ff
ff ff 64 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
```

#### 4.3.6 Comet — Fast Speed, Single Color Orange

```
02 02 08 50 00 00 32 50 02 00 00 00 00 00 32 ff
b9 00 64 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
```

## 5. Per-Key (Customize) Mode

In Per-Key (Customize) Mode, keys can be configured either **individually** (one bit set in 02 01 per \[02 01 + 02 02\] pair) or in **shared-effect key groups** (multiple bits set in a single 02 01 mask when all selected keys share the exact same effect and color parameters).

### 5.1 Bitmask Addressing

In `02 01` mask commands, bytes 2..33 represent a 32-byte (256-bit) little-endian bitfield where key bit index $k\in [0,255]$ maps to:

$$\text{Packet Byte Offset} = 2 + \lfloor k / 8 \rfloor, \quad \text{Bit Mask} = 1 \ll (k \bmod 8)$$

On the 99-key MSI Creator A16 AI+ keyboard, exactly **98 keys** have addressable RGB LEDs (the spacebar is unlit).

The bitwise OR union of all 98 valid key bits across Bytes 2..33 (Mask Bytes 0..31) is:

```
f0 ff ff ff ff ef fb ff 7f 92 ff ff 1f 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 7f 00 03 00
```

### 5.2 Key Bitmasks

- `Esc`: Bit 41

*Other keys have not yet been reverse-engineered.*

### 5.3 Full-Keyboard Serialization Order

When writing a full per-key customization layout:

1. *(On initial entry into Customize mode or when loading a new Customize preset)* Send the **Pre-Effect Global Clear** (see §3.2) and wait **~2.18–2.41 ms**

2. Apply iteratively for each key or group
    - **Individual Key Serialization**: 98 consecutive single-bit pairs are sent with a **0.50–1.55 ms** inter-key gap between `02 02` readback completion and the next `02 01` request.
    - **Grouped Bitmask Serialization**: Keys that share the exact same animation mode and color stops are combined into a single `02 01` bitmask

The official software serializes the 98 keys in 8 sweep groups using the following (Packet\_Byte\_Offset, Bitmask\_Hex) sequence:

1. **Group 1 (16 keys)**: `Esc` (7, 0x02), (9, 0x04), (9, 0x08), (9, 0x10), (9, 0x20), (9, 0x40), (9, 0x80), (10, 0x01), (10, 0x02), (10, 0x04), (10, 0x08), (10, 0x10), (10, 0x20), (10, 0x40), (11, 0x02), (11, 0x10)
2. **Group 2 (10 keys)**: (12, 0x20), (12, 0x10), (32, 0x02), (12, 0x04), (12, 0x01), (12, 0x02), (11, 0x80), (12, 0x80), (12, 0x40), (12, 0x08)
3. **Group 3 (12 keys)**: (13, 0x80), (14, 0x01), (14, 0x02), (13, 0x10), (13, 0x20), (13, 0x40), (13, 0x02), (13, 0x04), (13, 0x08), (14, 0x04), (14, 0x08), (13, 0x01)
4. **Group 4 (15 keys)**: (8, 0x20), (5, 0x40), (5, 0x80), (6, 0x01), (6, 0x02), (6, 0x04), (6, 0x08), (6, 0x10), (6, 0x20), (6, 0x40), (6, 0x80), (7, 0x20), (7, 0x40), (7, 0x04), (7, 0x08)
5. **Group 5 (14 keys)**: (4, 0x10), (5, 0x04), (3, 0x01), (4, 0x20), (4, 0x80), (5, 0x10), (5, 0x01), (3, 0x10), (4, 0x04), (4, 0x08), (7, 0x80), (8, 0x01), (8, 0x02), (9, 0x02)
6. **Group 6 (13 keys)**: (2, 0x10), (4, 0x40), (2, 0x80), (3, 0x02), (3, 0x04), (3, 0x08), (3, 0x20), (3, 0x40), (3, 0x80), (8, 0x08), (8, 0x10), (7, 0x01), (30, 0x02)
7. **Group 7 (10 keys)**: (5, 0x20), (5, 0x08), (2, 0x40), (5, 0x02), (2, 0x20), (4, 0x02), (4, 0x01), (8, 0x40), (8, 0x80), (9, 0x01)
8. **Group 8 (8 keys)**: (30, 0x20), (30, 0x01), (30, 0x08), (30, 0x04), (30, 0x40), (14, 0x10), (30, 0x10), (32, 0x01)

### 5.4 Per-Key (Customize) 02 02 Header Parameters

When configuring individual keys or key groups in Customize mode, use the following per-key `02 02` headers (Bytes 0..10), which differ slightly from their global-mode counterparts in period\_ms, direction\_x, density\_param, speed\_step, or sub\_mode\_flag:

| Per-Key Effect | Observed Customize Header (Bytes 0..10) | Key Differences vs. Global Header | Color Stop Template (Bytes 11..38) |
| :---- | :---- | :---- | :---- |
| **Steady (Per-Key)** | 02 02 01 64 00 32 32 0a 01 00 00 *(or 02 02 01 2c 01 32 32 0a 00 00 00)* | period\_ms = 100 (64 00) or 300 (2c 01) instead of 9c ff; Step = 0x01 or 0x00 | **Template 3.5.1** (\[R,G,B,64, R,G,B,00\]) |
| **Breath (Per-Key, Med)** | 02 02 02 20 03 00 32 50 02 00 00 | DirX = 0x00, Param = 0x50 (80), Step = 0x02 | **Template 3.5.2** (FG@32, BG@64, FG@00) |
| **Color Cycle (Per-Key, Med)** | 02 02 03 20 03 32 32 0a 00 00 00 | Step = 0x00 (vs. global 0x01) | **Template 3.5.3.1** (3-Color) or **Template 3.5.3.2** (Rainbow) |
| **Wave (Per-Key, Med)** | 02 02 04 f4 01 32 32 0a 00 01 00 | DirX = 0x32 (center), Step = 0x00, Sub = 0x01 | **Template 3.5.3.1** (3-Color) or **Template 3.5.3.2** (Rainbow) |
| **Dance (Per-Key, Fast 1s)** | 02 02 00 64 00 32 32 0a 01 00 00 *(or 02 02 00 64 00 32 32 0a 00 01 00) (or 02 02 00 64 00 00 32 50 02 01 00)* | Uses Step = 0x01, Sub = 0x00 in full-key Dance sweeps, or Sub = 0x01 in mixed layouts | All 0x00 |
| **Reactive (Per-Key, Med)** | 02 02 05 2c 01 00 32 50 02 00 00 *(or 02 02 05 2c 01 32 32 0a 00 00 00)* | Either matches global (DirX=00, Param=50, Step=02) or uses DirX=32, Param=0a, Step=00 | **Template 3.5.4.1** (00 00 00 32 R G B 64) |
| **Ripple (Per-Key, Med)** | 02 02 07 2c 01 00 32 0a 02 00 00 *(or 02 02 07 2c 01 32 32 0a 00 00 00)* | Either matches global (DirX=00, Step=02) or uses DirX=32, Step=00 | **Template 3.5.4.2** (Rainbow) or **Template 3.5.4.1** (Single) |
| **Unassigned / Disabled Key** | 02 02 00 64 00 32 32 0a 00 00 00 *(or 02 02 00 64 00 00 32 50 02 00 00)* | effect\_id = 0x00, Sub = 0x00, all color stops zeroed | All 0x00 |

> [!NOTE]
> **Hardware Behavior Uncertainties in Per-Key (Customize) Mode**:
> 1. **Per-Key Dance vs. Unassigned / Disabled Keys**: In Customize sweeps, unassigned keys without an active color assignment are transmitted with `effect_id = 0x00`, `sub_mode_flag = 0x00`, and all-zero color stops (`02 02 00 64 00 32 32 0a 00 00 00...` or `02 02 00 64 00 00 32 50 02 00 00...`). Active 1s Dance mode across keys also uses `effect_id = 0x00` with all-zero color stops (`02 02 00 64 00 32 32 0a 01 00 00...`). It remains uncertain whether the firmware distinguishes active per-key Dance from unassigned/disabled keys solely via `speed_step` (`0x01` vs. `0x00`) and `sub_mode_flag` (`0x01` vs. `0x00`), or whether assigning `effect_id = 0x06` (Off) directly to an individual key bitmask is accepted and supported by the firmware.
> 2. **Discard (`02 b0`) Firmware Handling**: When switching profiles or discarding preview changes, `02 b0` is sent with 62 trailing zeros. It is uncertain whether the firmware autonomously reloads the last committed profile from flash upon receiving `02 b0`, or if the host application is expected to follow up with explicit profile re-query (`01 a0`) and re-transmission of the stored lighting state.

## 6. Real-Time Music Mode

Music Mode turns the keyboard into a real-time host-driven audio visualizer (VU meter). Unlike autonomous lighting effects, the host application must continuously stream audio visualization frames to the keyboard.

### 6.1 Music Mode Initialization

1. **Send Pre-Effect Global Clear** (see §3.2)
2. **Wait ~146 ms**
3. **Initialize Music Color Palette (02 05)**:
    - Send SET\_REPORT(0x0302) with the 64-byte `02 05 ...` initialization payload below, followed immediately by GET\_REPORT(0x0302) (wLength = 65 bytes):
        ```
        02 05 00 00 00 00 ff 00 00 ff 00 ff ff 00 ff ff
        00 ff 00 00 ff 00 00 00 00 00 00 00 00 00 00 00
        00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
        00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
        ```
        - Bytes 0..1: `02 05`
        - Bytes 2..4: `00 00 00`
        - Bytes 5..19: Five 3-byte RGB color stops defining the VU-meter gradient: Green (`00 ff 00`), Green (`00 ff 00`), Yellow (`ff ff 00`), Yellow (`ff ff 00`), Red (`ff 00 00`)
        - Byte 20: `ff` (palette terminator)
        - Bytes 21..63: `00` padding
4. **Wait ~717 ms** after the initialization 65-byte GET\_REPORT(0x0302) completes before proceeding to Music Mode Streaming

### 6.2 Music Mode Streaming

The host application must continuously stream audio visualization frames to the keyboard every **~15.2 ms (~65.5 Hz)**.

While Music Mode is active:
1. Send Audio Frame (`02 06`) with Bytes 2..63 containing the audio visualization frame:
    ```
    02 06 <62 bytes of real-time audio visualization data>
    ```
    - *The audio visualization spectrum data has not yet been reverse-engineered.*
2. Issue GET\_REPORT(0x0302) (wLength = 65 bytes)
3. Sleep **7.89–10.65 ms** before transmitting the next `02 06` frame so that the request-to-request interval remains **~15.0–15.3 ms**
    - ~2.6 ms SET\_REPORT + ~3.9 ms GET\_REPORT = ~6.5 ms total USB transaction time

### 6.3 Music Mode Exit

1. Stop the `02 06` streaming loop
2. Wait ~115 ms
3. Transmit any standard `02 01` + `02 02` lighting command pair (switching back to GET\_REPORT(0x0301) 64-byte readbacks)
