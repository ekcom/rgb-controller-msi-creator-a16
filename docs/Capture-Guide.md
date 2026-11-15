# USB Packet Capture & Annotation Guide

This guide describes how to capture USB HID traffic from the official MSI software and format it into a `.pcapng` and `.csv` trace for the protocol conformance test suite in [`tests/pcap_conformance.rs`](tests/pcap_conformance.rs).

## 1. Dependencies

Because the official keyboard configuration software runs on Windows, captures are typically taken on a Windows host (or a Windows virtual machine with USB device passthrough enabled).

### Required Software:

- **Wireshark** with **USBPcap** installed
   -  During Wireshark installation on Windows, ensure the **"Install USBPcap"** checkbox is selected.
- **MSI Center**

## 2. Capturing USB Traffic in Wireshark

1. **Locate the USB Hub/Port**:
   1. Open Wireshark as Administrator.
   2. In the interface list, USBPcap will display one or more interfaces (e.g., `USBPcap1`, `USBPcap2`).
   3. Identify which USB root hub hosts the MSI keyboard controller (VID `1462`, PID `1565`). You can check Device Manager or launch a quick capture on each USBPcap interface while toggling keyboard effects until you see traffic.

2. **Configure Display Filters**:
   1. In Wireshark, set a display filter for the keyboard controller:
     ```
     usb.idVendor == 0x1462 && usb.idProduct == 0x1565
     ```
   2. Alternatively, filter by the controller's negotiated USB device address:
     ```
     usb.device_address == 1
     ```

3. **Start Capture**:
   1. Begin packet capture before launching or interacting with MSI Center.
   2. Pause 1–2 seconds between each action in the UI to keep transaction bursts visually separated by clear timestamp gaps.

## 3. Labeling Scenarios with Comments

The conformance test loader ([`PcapTraceLoader`](/pcap_conformance.rs#L53)) uses Wireshark **Packet Comments** (`frame.comment`) as scenario boundary markers. Every scenario begins when a packet containing a comment is encountered.

### How to Add a Comment in Wireshark:

1. Locate the **first packet** of the transaction burst:
   - Look for the Setup stage packet (`control_stage == 0.0`) of the initial `SET_IDLE` or `SET_REPORT`.
2. Right-click the packet row $\rightarrow$ select **"Packet Comment..."** (or press `Ctrl+Alt+C`).
3. Enter a clear, descriptive label representing the action performed in the OEM software.

### Recommended Labeling Conventions:

| Action | Example Comment Text |
| :--- | :--- |
| **Session Start** | `"start wireshark"`, `"Application Startup"` |
| **Static Color** | `"Set Steady Red"`, `"Set Steady #00FFCC"` |
| **Breathing** | `"Set Breath Cyan White, speed slow"` |
| **Wave** | `"Set Wave Rainbow LTR, speed fast"` |
| **Color Cycle** | `"Set Color Cycle Rainbow, speed medium"` |
| **Dance** | `"Set Dance Fast"` |
| **Autonomous Effect** | `"Set StarLight Rainbow, speed fast"`, `"Set Firework Cyan"` |
| **Reactive Effect** | `"Set Reactive White, speed fast"`, `"Set Ripple Cyan"` |
| **Per-Key Customization** | `"Set Customize: esc white rest dance"` |
| **Flash Persistence** | `"Save to Flash"` |

> [!IMPORTANT]
> **Always comment on the first packet** (`Setup` stage, `control_stage == 0.0`). Do not comment on the completion stage (`control_stage == 3.0`) or intermediate packets, as the scenario parser pairs transactions in lockstep starting from setup requests.

> [!TIP]
> It is recommended to **add comments as you capture** so that the sequence of actions performed is correct.

## 4. Saving the Capture

1. In Wireshark, select **File $\rightarrow$ Save As...**
2. Choose **Wireshark/… pcapng (*.pcapng)** as the file format.
3. Save the file to `tests/support/capture.pcapng`

## 5. Exporting the Test Fixture (`capture.csv`)

To ensure tests execute in milliseconds and run in CI environments without requiring `tshark` on the host, export the golden CSV fixture:

```bash
tshark -r tests/support/capture.pcapng \
  -T fields -E header=y -E separator=, -E quote=d \
  -e frame.number \
  -e frame.time_relative \
  -e frame.comment \
  -e usb.device_address \
  -e usb.control_stage \
  -e usb.data_len \
  -e usbhid.setup.bRequest \
  -e usbhid.setup.wValue \
  -e usbhid.setup.wLength \
  -e usb.data_fragment \
  > tests/support/capture.csv
```

*(Note: If `tests/support/capture.csv` is missing or deleted, [`tests/pcap_conformance.rs`](tests/pcap_conformance.rs) will automatically run this command to regenerate it if `tshark` is installed).*

## 6. Updating tests

Update `build_driver_test_cases` in [`tests/pcap_conformance.rs`](/tests/pcap_conformance.rs) with the new frame numbers and corresponding actions (`(start_frame, normalize_residuals, driver_action)`).

Every commented packet must have an associated test or be listed in `EXCLUDED_SCENARIOS`.

## 7. Verifying Conformance

Run the test suite to verify that the newly captured scenarios conform to the protocol state machine and match the driver:

```bash
cargo test --test pcap_conformance
```
