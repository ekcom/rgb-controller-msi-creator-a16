use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use rgb_controller_msi_creator_a16::driver::{
    CyclePalette, MsiKeyboardDriver, PerKeyEffect, ProfileTargetMode, Rgb, Speed, TransientPalette,
    WaveDirection,
};
use rgb_controller_msi_creator_a16::protocol::*;
use rgb_controller_msi_creator_a16::transport::{
    HidControlTransport, RecordedOperation, TransportError,
};
use serde::Deserialize;

const CAPTURE_CSV_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/capture.csv");
const CAPTURE_PCAPNG_PATH: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/capture.pcapng");

static SCENARIOS: OnceLock<Vec<CapturedScenario>> = OnceLock::new();

fn get_captured_scenarios() -> &'static [CapturedScenario] {
    SCENARIOS.get_or_init(|| {
        let csv_path = Path::new(CAPTURE_CSV_PATH);
        if csv_path.exists() {
            PcapTraceLoader::load_from_csv(csv_path)
        } else {
            PcapTraceLoader::load_from_pcapng(CAPTURE_PCAPNG_PATH)
        }
    })
}

// ============================================================================
// 1. Tshark / PCAPNG CSV Row Deserialization & Scenario Slicing
// ============================================================================

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // todo (loose) timing checks with time_relative
struct CsvFrameRow {
    #[serde(rename = "frame.number")]
    frame_number: u32,
    #[serde(rename = "frame.time_relative")]
    time_relative: f64,
    #[serde(rename = "frame.comment")]
    comment: Option<String>,
    #[serde(rename = "usb.device_address")]
    device_address: u16,
    #[serde(rename = "usb.control_stage")]
    control_stage: Option<f64>,
    #[serde(rename = "usb.data_len")]
    data_len: usize,
    #[serde(rename = "usbhid.setup.bRequest")]
    hid_b_request: Option<String>,
    #[serde(rename = "usbhid.setup.wValue")]
    hid_w_value: Option<String>,
    #[serde(rename = "usbhid.setup.wLength")]
    hid_w_length: Option<f64>,
    #[serde(rename = "usb.data_fragment")]
    data_fragment: Option<String>,
}

/// A single captured Scenario sliced from the trace between two `frame.comment` annotations.
#[derive(Debug, Clone)]
pub struct CapturedScenario {
    pub start_frame: u32,
    pub comment: String,
    pub operations: Vec<RecordedOperation>,
}

pub struct PcapTraceLoader;

impl PcapTraceLoader {
    /// Optional helper: transforms a raw `.pcapng` file into a CSV via `tshark` before loading.
    pub fn load_from_pcapng(pcapng_path: impl AsRef<Path>) -> Vec<CapturedScenario> {
        let path_ref = pcapng_path.as_ref();

        let output = Command::new("tshark")
            .args([
                "-r",
                path_ref.to_str().expect("Path must be valid UTF-8"),
                "-T",
                "fields",
                "-E",
                "header=y",
                "-E",
                "separator=,",
                "-E",
                "quote=d",
                "-e",
                "frame.number",
                "-e",
                "frame.time_relative",
                "-e",
                "frame.comment",
                "-e",
                "usb.device_address",
                "-e",
                "usb.control_stage",
                "-e",
                "usb.data_len",
                "-e",
                "usbhid.setup.bRequest",
                "-e",
                "usbhid.setup.wValue",
                "-e",
                "usbhid.setup.wLength",
                "-e",
                "usb.data_fragment",
            ])
            .output()
            .expect("Failed to execute tshark on .pcapng file");

        assert!(
            output.status.success(),
            "tshark exited with error (status: {}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        Self::parse_csv_bytes(&output.stdout)
    }

    /// Loads and segments capture into discrete `CapturedScenario` slices.
    pub fn load_from_csv(csv_path: impl AsRef<Path>) -> Vec<CapturedScenario> {
        let p = csv_path.as_ref();
        let bytes = std::fs::read(p).unwrap_or_else(|_| panic!("Failed to read {}", p.display()));
        Self::parse_csv_bytes(&bytes)
    }

    pub fn parse_csv_bytes(csv_bytes: &[u8]) -> Vec<CapturedScenario> {
        let mut rdr = csv::Reader::from_reader(csv_bytes);
        let all_rows: Vec<CsvFrameRow> = rdr
            .deserialize::<CsvFrameRow>()
            .map(|r| r.expect("Invalid CSV row"))
            .collect();

        // Dynamically identify the keyboard controller device address by finding
        // the device that executes USB HID control requests (bRequest is present).
        let Some(target_dev_addr) = all_rows
            .iter()
            .find(|r| r.hid_b_request.is_some())
            .map(|r| r.device_address)
        else {
            return Vec::new();
        };

        // Dynamically determine the first frame where HID communication begins,
        // skipping low-level bus descriptor enumeration frames (GET_DESCRIPTOR).
        let start_frame = all_rows
            .iter()
            .find(|r| r.device_address == target_dev_addr && r.hid_b_request.is_some())
            .map(|r| r.frame_number)
            .unwrap_or(0);

        let rows: Vec<CsvFrameRow> = all_rows
            .into_iter()
            .filter(|r| r.device_address == target_dev_addr && r.frame_number >= start_frame)
            .collect();

        let mut scenarios: Vec<CapturedScenario> = Vec::new();
        let mut current_scenario: Option<CapturedScenario> = None;

        for pair in rows.chunks_exact(2) {
            let req = &pair[0];
            let resp = &pair[1];
            assert_eq!(req.control_stage, Some(0.0), "Expected Setup stage (0.0)");
            assert_eq!(
                resp.control_stage,
                Some(3.0),
                "Expected Complete stage (3.0)"
            );

            // Start a new scenario whenever a frame comment is present
            if let Some(comment_text) = req
                .comment
                .as_deref()
                .map(str::trim)
                .filter(|c| !c.is_empty())
            {
                if let Some(prev) = current_scenario.take() {
                    scenarios.push(prev);
                }
                current_scenario = Some(CapturedScenario {
                    start_frame: req.frame_number,
                    comment: comment_text.to_string(),
                    operations: Vec::new(),
                });
            }

            let b_req = parse_hex_u8(req.hid_b_request.as_deref().unwrap());
            let w_val = parse_hex_u16(req.hid_w_value.as_deref().unwrap());
            let w_len = req.hid_w_length.unwrap_or(0.0) as u16;

            let op = match b_req {
                HID_REQ_SET_IDLE => RecordedOperation::SetIdle,
                HID_REQ_SET_REPORT => {
                    let hex_str = req.data_fragment.as_deref().expect("Missing payload");
                    let payload = decode_hex_64(hex_str);
                    RecordedOperation::SetReport {
                        w_value: w_val,
                        payload,
                    }
                }
                HID_REQ_GET_REPORT => {
                    assert_eq!(
                        resp.data_len,
                        usize::from(w_len),
                        "GET_REPORT response data_len must equal wLength at frame {}",
                        resp.frame_number
                    );
                    RecordedOperation::GetReport {
                        w_value: w_val,
                        length: w_len,
                    }
                }
                other => panic!(
                    "Unexpected bRequest 0x{other:02x} at frame {}",
                    req.frame_number
                ),
            };

            if let Some(scen) = current_scenario.as_mut() {
                scen.operations.push(op);
            }
        }

        if let Some(last) = current_scenario {
            scenarios.push(last);
        }
        scenarios
    }
}

// ============================================================================
// 2. Stateful Protocol Invariant Validator (State-Machine Auditor)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BusState {
    Idle,
    AwaitingStandardGetReport { preceding_opcode: (u8, u8) },
    AwaitingMusicGetReport { preceding_opcode: (u8, u8) },
}

pub struct ProtocolValidatorTransport {
    pub recorded: Vec<RecordedOperation>,
    bus_state: BusState,
    pending_key_mask: Option<[u8; 32]>,
    last_cleared_all_zero: bool,
    pending_first_flash_apply: bool,
    accumulated_sleep_since_flash_ms: u128,
}

impl Default for ProtocolValidatorTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolValidatorTransport {
    pub fn new() -> Self {
        Self {
            recorded: Vec::new(),
            bus_state: BusState::Idle,
            pending_key_mask: None,
            last_cleared_all_zero: false,
            pending_first_flash_apply: false,
            accumulated_sleep_since_flash_ms: 0,
        }
    }

    /// Asserts that no incomplete lockstep pairs or single-cycle flash writes remain open.
    pub fn assert_clean_idle_state(&self) {
        assert_eq!(
            self.bus_state,
            BusState::Idle,
            "Protocol violation: Bus left with an unacknowledged SET_REPORT ({:?})",
            self.bus_state
        );
        assert!(
            self.pending_key_mask.is_none(),
            "Protocol violation: 02 01 Key Mask was sent without a following 02 02 Set Effect"
        );
        assert!(
            !self.pending_first_flash_apply,
            "Protocol violation: 02 a0 Flash Apply was only sent once (must always be sent twice)"
        );
    }
}

impl HidControlTransport for ProtocolValidatorTransport {
    fn set_idle(&mut self) -> Result<(), TransportError> {
        assert_eq!(
            self.bus_state,
            BusState::Idle,
            "Protocol violation: SET_IDLE issued while awaiting GET_REPORT"
        );
        self.recorded.push(RecordedOperation::SetIdle);
        Ok(())
    }

    fn set_report(
        &mut self,
        w_value: u16,
        payload: &[u8; REPORT_LEN],
    ) -> Result<(), TransportError> {
        assert_eq!(
            self.bus_state,
            BusState::Idle,
            "Protocol violation: SET_REPORT issued before preceding GET_REPORT completed"
        );

        let report_id = payload[0];
        let opcode = payload[1];

        // Invariant 1: wValue must match Byte 0 (Report ID)
        match report_id {
            REPORT_ID_1 => assert_eq!(
                w_value, WVALUE_REPORT_ID_1,
                "Report ID 1 payload must use wValue = 0x0301"
            ),
            REPORT_ID_2 => assert_eq!(
                w_value, WVALUE_REPORT_ID_2,
                "Report ID 2 payload must use wValue = 0x0302"
            ),
            other => panic!("Invalid Report ID 0x{other:02x} in Byte 0"),
        }

        // Invariant 2: Validate command-specific structure and ordering
        match (report_id, opcode) {
            (REPORT_ID_1, CMD_HANDSHAKE_91) => {
                assert_eq!(payload[2], 0x01, "01 91 Byte 2 must be 0x01");
                assert!(
                    payload[3..].iter().all(|&b| b == 0),
                    "01 91 Bytes 3..63 must be 0x00"
                );
                self.bus_state = BusState::AwaitingStandardGetReport {
                    preceding_opcode: (report_id, opcode),
                };
            }
            (REPORT_ID_1, CMD_STATE_QUERY_A0) => {
                assert!(
                    payload[2..].iter().all(|&b| b == 0),
                    "01 a0 Bytes 2..63 must be 0x00"
                );
                self.bus_state = BusState::AwaitingStandardGetReport {
                    preceding_opcode: (report_id, opcode),
                };
            }
            (REPORT_ID_2, SUBCMD_KEY_MASK) => {
                assert!(
                    self.pending_key_mask.is_none(),
                    "Protocol violation: Two consecutive 02 01 Key Mask commands without 02 02"
                );
                assert!(
                    payload[34..].iter().all(|&b| b == 0),
                    "02 01 Bytes 34..63 must be 0x00"
                );
                let mut mask = [0u8; 32];
                mask.copy_from_slice(&payload[2..34]);

                // Key mask must either be global (ff*32) or a non-empty subset of VALID_98_KEY_MASK
                if mask != GLOBAL_KEY_MASK {
                    let mut any_bit = false;
                    for i in 0..32 {
                        assert_eq!(
                            mask[i] & !VALID_98_KEY_MASK[i],
                            0,
                            "02 01 Mask Byte {i} (Packet Byte {}) sets invalid unpopulated key bit: 0x{:02x}",
                            i + 2,
                            mask[i]
                        );
                        if mask[i] != 0 {
                            any_bit = true;
                        }
                    }
                    assert!(any_bit, "Per-key 02 01 bitmask must select at least 1 key");
                }

                self.pending_key_mask = Some(mask);
                self.bus_state = BusState::AwaitingStandardGetReport {
                    preceding_opcode: (report_id, opcode),
                };
            }
            (REPORT_ID_2, SUBCMD_SET_EFFECT) => {
                let mask = self.pending_key_mask.take().expect(
                    "Protocol violation: 02 02 Set Effect issued without preceding 02 01 Key Mask",
                );

                assert_eq!(payload[10], 0x00, "02 02 Byte 10 (reserved) must be 0x00");
                assert!(
                    payload[39..].iter().all(|&b| b == 0),
                    "02 02 Bytes 39..63 padding must be 0x00"
                );

                let effect_id = payload[2];
                let is_all_zero_clear =
                    effect_id == EFFECT_OFF && payload[3..39].iter().all(|&b| b == 0);

                // Global Dance (02 02 00 with global mask) must be preceded by all-zero Off clear
                if mask == GLOBAL_KEY_MASK && effect_id == EFFECT_DANCE {
                    assert!(
                        self.last_cleared_all_zero,
                        "Protocol violation: Global Dance (02 02 00) must be preceded by 02 02 06 00... global clear"
                    );
                }

                self.last_cleared_all_zero = mask == GLOBAL_KEY_MASK && is_all_zero_clear;
                self.bus_state = BusState::AwaitingStandardGetReport {
                    preceding_opcode: (report_id, opcode),
                };
            }
            (REPORT_ID_2, SUBCMD_MUSIC_INIT) => {
                assert!(
                    self.last_cleared_all_zero,
                    "Protocol violation: Music Init (02 05) must be preceded by 02 02 06 00... global clear"
                );
                assert_eq!(payload[20], 0xff, "02 05 Byte 20 terminator must be 0xff");
                assert!(
                    payload[21..].iter().all(|&b| b == 0),
                    "02 05 Bytes 21..63 must be 0x00"
                );
                self.bus_state = BusState::AwaitingMusicGetReport {
                    preceding_opcode: (report_id, opcode),
                };
            }
            (REPORT_ID_2, SUBCMD_MUSIC_FRAME) => {
                self.bus_state = BusState::AwaitingMusicGetReport {
                    preceding_opcode: (report_id, opcode),
                };
            }
            (REPORT_ID_2, SUBCMD_FLASH_APPLY) => {
                assert!(
                    payload[2..].iter().all(|&b| b == 0),
                    "02 a0 Bytes 2..63 must be 0x00"
                );
                if !self.pending_first_flash_apply {
                    self.pending_first_flash_apply = true;
                    self.accumulated_sleep_since_flash_ms = 0;
                } else {
                    assert!(
                        self.accumulated_sleep_since_flash_ms >= 80,
                        "Protocol violation: Second 02 a0 Flash Apply sent after only {} ms pause (requires >= 80 ms)",
                        self.accumulated_sleep_since_flash_ms
                    );
                    self.pending_first_flash_apply = false;
                }
                self.bus_state = BusState::AwaitingStandardGetReport {
                    preceding_opcode: (report_id, opcode),
                };
            }
            (REPORT_ID_2, SUBCMD_DISCARD) => {
                assert!(
                    payload[2..].iter().all(|&b| b == 0),
                    "02 b0 Bytes 2..63 must be 0x00"
                );
                self.bus_state = BusState::AwaitingStandardGetReport {
                    preceding_opcode: (report_id, opcode),
                };
            }
            other => panic!("Unknown command prefix {:02x?}", other),
        }

        self.recorded.push(RecordedOperation::SetReport {
            w_value,
            payload: *payload,
        });
        Ok(())
    }

    fn get_report(&mut self, w_value: u16, length: u16) -> Result<Vec<u8>, TransportError> {
        match self.bus_state {
            BusState::Idle => {
                panic!("Protocol violation: GET_REPORT issued without a preceding SET_REPORT");
            }
            BusState::AwaitingStandardGetReport { preceding_opcode } => {
                assert_eq!(
                    w_value, WVALUE_REPORT_ID_1,
                    "Standard command {:02x?} requires GET_REPORT(0x0301), got 0x{w_value:04x}",
                    preceding_opcode
                );
                assert_eq!(
                    length, 64,
                    "Standard GET_REPORT(0x0301) requires wLength = 64, got {length}"
                );
            }
            BusState::AwaitingMusicGetReport { preceding_opcode } => {
                assert_eq!(
                    w_value, WVALUE_REPORT_ID_2,
                    "Music command {:02x?} requires GET_REPORT(0x0302), got 0x{w_value:04x}",
                    preceding_opcode
                );
                assert_eq!(
                    length, 65,
                    "Music GET_REPORT(0x0302) requires wLength = 65, got {length}"
                );
            }
        }

        self.bus_state = BusState::Idle;
        self.recorded
            .push(RecordedOperation::GetReport { w_value, length });
        Ok(vec![0u8; usize::from(length)])
    }

    fn sleep(&mut self, duration: Duration) {
        if self.pending_first_flash_apply {
            self.accumulated_sleep_since_flash_ms += duration.as_millis();
        }
        self.recorded.push(RecordedOperation::Sleep(duration));
    }
}

// ============================================================================
// 3. Semantic Trace Comparison & Integration Tests Against Empirical Capture
// ============================================================================

/// Normalizes known uninitialized-buffer artifacts in the official MSI software capture
/// so clean zero-padded payloads can be compared against captured wire packets:
/// - Global Dance (`02 02 00`): zeroes residual color stops in Bytes 11..38.
/// - Explicit Off (`02 02 06`) with residual non-zero stops (Frames 793 & 1509): normalizes to standard Off.
fn normalize_captured_op(op: &RecordedOperation) -> RecordedOperation {
    match op {
        RecordedOperation::SetReport { w_value, payload } => {
            let mut norm = *payload;
            if norm[0] == 0x02 && norm[1] == 0x02 {
                let effect_id = norm[2];
                if effect_id == EFFECT_DANCE {
                    // Global Dance ignores residual color stops left over from Wave
                    norm[11..39].fill(0x00);
                } else if effect_id == EFFECT_OFF && norm[11..39].iter().any(|&b| b != 0) {
                    // Frames 793 ("Set Off") and 1509 ("Close app Off") carry residual bytes from Comet/Reactive
                    norm = rgb_controller_msi_creator_a16::driver::build_off_standard_packet();
                }
            }
            RecordedOperation::SetReport {
                w_value: *w_value,
                payload: norm,
            }
        }
        other => other.clone(),
    }
}

/// Strips `SetIdle` and `Sleep` operations to compare the sequence of `SET_REPORT` + `GET_REPORT` pairs.
fn extract_bus_reports(
    ops: &[RecordedOperation],
    normalize_residuals: bool,
) -> Vec<RecordedOperation> {
    ops.iter()
        .filter(|op| {
            matches!(
                op,
                RecordedOperation::SetReport { .. } | RecordedOperation::GetReport { .. }
            )
        })
        .map(|op| {
            if normalize_residuals {
                normalize_captured_op(op)
            } else {
                op.clone()
            }
        })
        .collect()
}

fn parse_hex_u8(s: &str) -> u8 {
    u8::from_str_radix(s.trim_start_matches("0x"), 16).unwrap()
}

fn parse_hex_u16(s: &str) -> u16 {
    u16::from_str_radix(s.trim_start_matches("0x"), 16).unwrap()
}

fn decode_hex_64(hex: &str) -> [u8; 64] {
    assert_eq!(hex.len(), 128, "Expected 128 hex chars (64 bytes)");
    let mut out = [0u8; 64];
    for i in 0..64 {
        out[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap();
    }
    out
}

#[test]
fn test_entire_captured_csv_conforms_to_protocol_state_machine() {
    let scenarios = get_captured_scenarios();

    // Replay every single captured transaction through ProtocolValidatorTransport
    // to ensure the state machine rules match every frame
    let mut validator = ProtocolValidatorTransport::new();
    for scen in scenarios {
        for op in &scen.operations {
            match op {
                RecordedOperation::SetIdle => validator.set_idle().unwrap(),
                RecordedOperation::SetReport { w_value, payload } => {
                    // Provide synthetic inter-cycle sleep before the second 02 a0 in each Apply scenario
                    if payload[0] == 0x02 && payload[1] == SUBCMD_FLASH_APPLY {
                        validator.sleep(Duration::from_millis(90));
                    }
                    validator.set_report(*w_value, payload).unwrap();
                }
                RecordedOperation::GetReport { w_value, length } => {
                    validator.get_report(*w_value, *length).unwrap();
                }
                RecordedOperation::Sleep(d) => validator.sleep(*d),
            }
        }
        validator.assert_clean_idle_state();
    }
}

type DriverAction = Box<dyn FnOnce(&mut MsiKeyboardDriver<ProtocolValidatorTransport>)>;

/// Canonical driver invocations tested byte-for-byte against captured wire traffic.
fn build_driver_test_cases() -> Vec<(u32, bool, DriverAction)> {
    vec![
        (
            19,
            false,
            Box::new(|d| {
                let off = rgb_controller_msi_creator_a16::driver::build_off_standard_packet();
                d.initialize_session(true, &off).unwrap();
            }),
        ),
        (57, false, Box::new(|d| d.set_steady(Rgb::WHITE).unwrap())),
        (67, false, Box::new(|d| d.set_steady(Rgb::RED).unwrap())),
        (77, false, Box::new(|d| d.set_steady(Rgb::GREEN).unwrap())),
        (87, false, Box::new(|d| d.set_steady(Rgb::BLUE).unwrap())),
        (
            97,
            false,
            Box::new(|d| {
                d.set_breath(Rgb::WHITE, Rgb::WARM_ORANGE, Speed::Medium)
                    .unwrap()
            }),
        ),
        (
            117,
            false,
            Box::new(|d| {
                d.set_breath(Rgb::WHITE, Rgb::WARM_ORANGE, Speed::Slow)
                    .unwrap()
            }),
        ),
        (
            127,
            false,
            Box::new(|d| {
                d.set_breath(Rgb::WHITE, Rgb::WARM_ORANGE, Speed::Fast)
                    .unwrap()
            }),
        ),
        (
            147,
            false,
            Box::new(|d| d.set_breath(Rgb::WHITE, Rgb::GREEN, Speed::Fast).unwrap()),
        ),
        (
            157,
            false,
            Box::new(|d| d.set_breath(Rgb::RED, Rgb::GREEN, Speed::Fast).unwrap()),
        ),
        (
            167,
            false,
            Box::new(|d| {
                d.set_color_cycle(CyclePalette::Rainbow, Speed::Medium)
                    .unwrap()
            }),
        ),
        (
            213,
            false,
            Box::new(|d| {
                d.set_color_cycle(CyclePalette::Rainbow, Speed::Slow)
                    .unwrap()
            }),
        ),
        (
            223,
            false,
            Box::new(|d| {
                d.set_color_cycle(CyclePalette::Rainbow, Speed::Fast)
                    .unwrap()
            }),
        ),
        (
            273,
            false,
            Box::new(|d| {
                d.set_wave(
                    CyclePalette::Rainbow,
                    Speed::Medium,
                    WaveDirection::LeftToRight,
                )
                .unwrap()
            }),
        ),
        (
            283,
            false,
            Box::new(|d| {
                d.set_wave(
                    CyclePalette::Rainbow,
                    Speed::Medium,
                    WaveDirection::RightToLeft,
                )
                .unwrap()
            }),
        ),
        (
            313,
            false,
            Box::new(|d| {
                d.set_wave(
                    CyclePalette::ThreeColor(Rgb::WHITE, Rgb::WARM_ORANGE, Rgb::CYAN),
                    Speed::Fast,
                    WaveDirection::RightToLeft,
                )
                .unwrap()
            }),
        ),
        // Dance modes (normalize_residuals = true to ignore stale Wave color stops in Bytes 11..26)
        (333, true, Box::new(|d| d.set_dance(Speed::Fast).unwrap())),
        (351, true, Box::new(|d| d.set_dance(Speed::Medium).unwrap())),
        (369, true, Box::new(|d| d.set_dance(Speed::Slow).unwrap())),
        // StarLight, Firework, Laser, Reactive, Ripple, Comet
        (
            561,
            false,
            Box::new(|d| {
                d.set_starlight(TransientPalette::Rainbow, Speed::Slow)
                    .unwrap()
            }),
        ),
        (
            585,
            false,
            Box::new(|d| {
                d.set_starlight(TransientPalette::SingleColor(Rgb::ORANGE), Speed::Fast)
                    .unwrap()
            }),
        ),
        (
            601,
            false,
            Box::new(|d| {
                d.set_firework(TransientPalette::Rainbow, Speed::Medium)
                    .unwrap()
            }),
        ),
        (
            625,
            false,
            Box::new(|d| {
                d.set_firework(TransientPalette::SingleColor(Rgb::LIGHT_CYAN), Speed::Fast)
                    .unwrap()
            }),
        ),
        (
            633,
            false,
            Box::new(|d| {
                d.set_laser(TransientPalette::Rainbow, Speed::Medium)
                    .unwrap()
            }),
        ),
        (
            665,
            false,
            Box::new(|d| {
                d.set_reactive(TransientPalette::SingleColor(Rgb::WHITE), Speed::Medium)
                    .unwrap()
            }),
        ),
        (
            689,
            false,
            Box::new(|d| {
                d.set_ripple(TransientPalette::Rainbow, Speed::Medium)
                    .unwrap()
            }),
        ),
        (
            721,
            false,
            Box::new(|d| {
                d.set_comet(TransientPalette::Rainbow, Speed::Medium)
                    .unwrap()
            }),
        ),
        (
            781,
            false,
            Box::new(|d| {
                d.set_comet(TransientPalette::SingleColor(Rgb::ORANGE), Speed::Fast)
                    .unwrap()
            }),
        ),
        (789, true, Box::new(|d| d.set_off().unwrap())),
        (797, false, Box::new(|d| d.apply_to_flash(false).unwrap())),
        // App Lifecycle: Discard changes and turn off on app close
        (1501, true, Box::new(|d| d.discard_and_close().unwrap())),
        // App Lifecycle: Re-open app (session handshake with startup = false)
        (
            1513,
            false,
            Box::new(|d| {
                let off = rgb_controller_msi_creator_a16::driver::build_off_standard_packet();
                d.initialize_session(false, &off).unwrap();
            }),
        ),
        // Profile Switch: Switch from Profile 1 to Profile 2 (Dance Fast)
        (
            1549,
            false,
            Box::new(|d| {
                d.switch_profile(false, ProfileTargetMode::Dance { speed: Speed::Fast })
                    .unwrap();
            }),
        ),
        // Flash Apply after Profile 2
        (1597, false, Box::new(|d| d.apply_to_flash(false).unwrap())),
        // Profile Switch: Switch from Profile 2 to Profile 3 (Dance Fast)
        (
            1605,
            false,
            Box::new(|d| {
                d.switch_profile(false, ProfileTargetMode::Dance { speed: Speed::Fast })
                    .unwrap();
            }),
        ),
        // Profile Switch: Switch from Profile 3 to Profile 1 (discard changes, Mode Off)
        (
            1653,
            false,
            Box::new(|d| {
                d.switch_profile(true, ProfileTargetMode::Off).unwrap();
            }),
        ),
        // Profile Switch: Switch from Profile 1 to Profile 2 (discard changes, Dance Fast)
        (
            1689,
            false,
            Box::new(|d| {
                d.switch_profile(true, ProfileTargetMode::Dance { speed: Speed::Fast })
                    .unwrap();
            }),
        ),
        // Full 98-key Customize Sweep 4 (Frame 3317: Esc = Steady White, all other 97 keys = Dance Fast)
        (
            3317,
            false,
            Box::new(|d| {
                d.set_full_98_key_customize(false, |coord| {
                    if coord == KEY_ESC_COORD {
                        PerKeyEffect::Steady(Rgb::WHITE)
                    } else {
                        PerKeyEffect::Dance { speed: Speed::Fast }
                    }
                })
                .unwrap();
            }),
        ),
        // Flash Apply after Customize Sweep 4 (Customize layout persistence)
        (4101, false, Box::new(|d| d.apply_to_flash(true).unwrap())),
    ]
}

#[test]
fn test_driver_calls_match_captured_scenarios_byte_for_byte() {
    let scenarios = get_captured_scenarios();
    let by_frame: HashMap<u32, &CapturedScenario> =
        scenarios.iter().map(|s| (s.start_frame, s)).collect();

    let test_cases = build_driver_test_cases();

    for (frame_no, normalize_residuals, action) in test_cases {
        let captured = by_frame
            .get(&frame_no)
            .unwrap_or_else(|| panic!("Frame {frame_no} scenario not found"));

        let mut driver = MsiKeyboardDriver::new(ProtocolValidatorTransport::new());
        action(&mut driver);
        driver.transport().assert_clean_idle_state();

        let actual_ops = extract_bus_reports(&driver.transport().recorded, normalize_residuals);
        let expected_ops = extract_bus_reports(&captured.operations, normalize_residuals);

        assert_eq!(
            actual_ops, expected_ops,
            "Mismatch against captured scenario at Frame {frame_no} ('{}')",
            captured.comment
        );
    }
}

// ============================================================================
// 4. Trace Coverage Exhaustiveness & Wire Discrepancy Meta-Test
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExclusionReason {
    /// Redundant packet, particularly from GUI refreshing unenecessarily
    Reapplied,
    /// Annotation discrepancy on the wire
    AnnotationMismatch,
    /// Real-time audio FFT streaming frames (requires active audio feed)
    RealtimeAudioStream,
    /// Intermediate customize sweeps superseded by comprehensive 98-key test
    IntermediateCustomizeSweep,
    /// Redundant speed/palette variant covered by other canonical tests
    RedundantVariantCoveredElsewhere,
}

pub struct ExcludedScenario {
    pub start_frame: u32,
    pub reason: ExclusionReason,
    pub description: &'static str,
}

pub const EXCLUDED_SCENARIOS: &[ExcludedScenario] = &[
    ExcludedScenario {
        start_frame: 107,
        reason: ExclusionReason::Reapplied,
        description: "Duplicate refresh of Medium Breath",
    },
    ExcludedScenario {
        start_frame: 137,
        reason: ExclusionReason::Reapplied,
        description: "Duplicate refresh of Fast Breath",
    },
    ExcludedScenario {
        start_frame: 233,
        reason: ExclusionReason::AnnotationMismatch,
        description: "Mislabeled speed medium; OEM software transmitted Fast speed (period_ms = 90 01 / 400ms)",
    },
    ExcludedScenario {
        start_frame: 243,
        reason: ExclusionReason::Reapplied,
        description: "Duplicate re-send of Color Cycle 3-color settings",
    },
    ExcludedScenario {
        start_frame: 253,
        reason: ExclusionReason::Reapplied,
        description: "Duplicate re-send of Color Cycle 3-color settings",
    },
    ExcludedScenario {
        start_frame: 263,
        reason: ExclusionReason::Reapplied,
        description: "Duplicate reapply of Color Cycle 3-color settings",
    },
    ExcludedScenario {
        start_frame: 293,
        reason: ExclusionReason::RedundantVariantCoveredElsewhere,
        description: "Wave Rainbow Slow RTL (Wave mechanics verified at Frames 273, 283, 313)",
    },
    ExcludedScenario {
        start_frame: 303,
        reason: ExclusionReason::RedundantVariantCoveredElsewhere,
        description: "Wave Rainbow Fast RTL (Wave mechanics verified at Frames 273, 283, 313)",
    },
    ExcludedScenario {
        start_frame: 323,
        reason: ExclusionReason::Reapplied,
        description: "Duplicate reapply of 3-color Wave Fast RTL",
    },
    ExcludedScenario {
        start_frame: 387,
        reason: ExclusionReason::RealtimeAudioStream,
        description: "Music Mode initiation and streaming packets 401-552",
    },
    ExcludedScenario {
        start_frame: 569,
        reason: ExclusionReason::RedundantVariantCoveredElsewhere,
        description: "StarLight Rainbow Medium (StarLight verified at Frames 561, 585)",
    },
    ExcludedScenario {
        start_frame: 577,
        reason: ExclusionReason::RedundantVariantCoveredElsewhere,
        description: "StarLight Rainbow Fast (StarLight verified at Frames 561, 585)",
    },
    ExcludedScenario {
        start_frame: 593,
        reason: ExclusionReason::Reapplied,
        description: "Duplicate reapply of StarLight Orange Fast",
    },
    ExcludedScenario {
        start_frame: 609,
        reason: ExclusionReason::RedundantVariantCoveredElsewhere,
        description: "Firework Rainbow Slow (Firework verified at Frames 601, 625)",
    },
    ExcludedScenario {
        start_frame: 617,
        reason: ExclusionReason::RedundantVariantCoveredElsewhere,
        description: "Firework Rainbow Fast (Firework verified at Frames 601, 625)",
    },
    ExcludedScenario {
        start_frame: 641,
        reason: ExclusionReason::RedundantVariantCoveredElsewhere,
        description: "Laser Rainbow Slow (Laser verified at Frame 633)",
    },
    ExcludedScenario {
        start_frame: 649,
        reason: ExclusionReason::RedundantVariantCoveredElsewhere,
        description: "Laser Rainbow Fast (Laser verified at Frame 633)",
    },
    ExcludedScenario {
        start_frame: 657,
        reason: ExclusionReason::RedundantVariantCoveredElsewhere,
        description: "Laser Orange Fast (Laser verified at Frame 633)",
    },
    ExcludedScenario {
        start_frame: 673,
        reason: ExclusionReason::RedundantVariantCoveredElsewhere,
        description: "Reactive White Slow (Reactive verified at Frame 665)",
    },
    ExcludedScenario {
        start_frame: 681,
        reason: ExclusionReason::RedundantVariantCoveredElsewhere,
        description: "Reactive White Fast (Reactive verified at Frame 665)",
    },
    ExcludedScenario {
        start_frame: 697,
        reason: ExclusionReason::RedundantVariantCoveredElsewhere,
        description: "Ripple Rainbow Slow (Ripple verified at Frame 689)",
    },
    ExcludedScenario {
        start_frame: 705,
        reason: ExclusionReason::RedundantVariantCoveredElsewhere,
        description: "Ripple Rainbow Fast (Ripple verified at Frame 689)",
    },
    ExcludedScenario {
        start_frame: 713,
        reason: ExclusionReason::RedundantVariantCoveredElsewhere,
        description: "Ripple Cyan Fast (Ripple verified at Frame 689)",
    },
    ExcludedScenario {
        start_frame: 729,
        reason: ExclusionReason::RedundantVariantCoveredElsewhere,
        description: "Comet Rainbow Slow (Comet verified at Frames 721, 781)",
    },
    ExcludedScenario {
        start_frame: 773,
        reason: ExclusionReason::RedundantVariantCoveredElsewhere,
        description: "Comet Rainbow Fast (Comet verified at Frames 721, 781)",
    },
    ExcludedScenario {
        start_frame: 805,
        reason: ExclusionReason::IntermediateCustomizeSweep,
        description: "Customize Sweep 1: Elijah's layout (superseded by full 98-key test at Frame 3317)",
    },
    ExcludedScenario {
        start_frame: 1741,
        reason: ExclusionReason::IntermediateCustomizeSweep,
        description: "Customize Sweep 2: All keys Dance Fast (superseded by full 98-key test at Frame 3317)",
    },
    ExcludedScenario {
        start_frame: 2533,
        reason: ExclusionReason::IntermediateCustomizeSweep,
        description: "Customize Sweep 3: All keys Dance Fast, select Esc (superseded by full 98-key test at Frame 3317)",
    },
];

#[test]
fn test_scenario_coverage_and_discrepancy_audit() {
    let scenarios = get_captured_scenarios();
    let all_captured_frames: HashSet<u32> = scenarios.iter().map(|s| s.start_frame).collect();

    let driver_cases = build_driver_test_cases();
    let tested_frames: HashSet<u32> = driver_cases.iter().map(|(frame, _, _)| *frame).collect();

    let excluded_frames: HashSet<u32> = EXCLUDED_SCENARIOS.iter().map(|s| s.start_frame).collect();

    // 1. Every tested frame must exist in the capture
    for frame in &tested_frames {
        assert!(
            all_captured_frames.contains(frame),
            "Driver test case references Frame {frame}, which does not exist in the capture"
        );
    }

    // 2. Every excluded frame must exist in the capture
    for frame in &excluded_frames {
        assert!(
            all_captured_frames.contains(frame),
            "Exclusion list references Frame {frame}, which does not exist in the capture"
        );
    }

    // 3. Tested frames and excluded frames must be mutually exclusive (no overlap)
    let overlap: Vec<_> = tested_frames
        .intersection(&excluded_frames)
        .copied()
        .collect();
    assert!(
        overlap.is_empty(),
        "Frames appear in BOTH driver test cases and exclusion list: {overlap:?}"
    );

    // 4. Exact partition: tested + excluded must cover 100% of captured scenarios
    let union: HashSet<u32> = tested_frames.union(&excluded_frames).copied().collect();
    let unaccounted: Vec<_> = all_captured_frames.difference(&union).copied().collect();
    assert!(
        unaccounted.is_empty(),
        "Capture contains scenarios that are neither tested nor explicitly excluded: {unaccounted:?}"
    );

    // 5. Wire Discrepancy Audits

    // 5a. Discrepancy: Frame 233 speed annotation mismatch
    let frame_233_scen = scenarios
        .iter()
        .find(|s| s.start_frame == 233)
        .expect("Frame 233 missing");
    let frame_233_set_effect = frame_233_scen
        .operations
        .iter()
        .find_map(|op| {
            if let RecordedOperation::SetReport { w_value, payload } = op {
                if *w_value == 0x0302 && payload[1] == SUBCMD_SET_EFFECT {
                    return Some(payload);
                }
            }
            None
        })
        .expect("Frame 233 Set Effect missing");
    let period_233 = u16::from_le_bytes([frame_233_set_effect[3], frame_233_set_effect[4]]);
    assert_eq!(
        period_233, 400,
        "Audit check: Frame 233 comment says 'speed medium', but wire actually transmitted 400ms (Fast)"
    );

    // 5b. Discrepancy: Frame 553 unannotated mode transition anomaly
    // In raw capture, Frame 553 is an unannotated burst transmitting StarLight Fast immediately following Music mode.
    // Verify that Frame 387 (Music mode scenario) ends with the StarLight Fast transition packets (Frames 553-560).
    let music_scen = scenarios
        .iter()
        .find(|s| s.start_frame == 387)
        .expect("Music scenario missing");
    let trailing_set_effects: Vec<_> = music_scen
        .operations
        .iter()
        .filter_map(|op| {
            if let RecordedOperation::SetReport { w_value, payload } = op {
                if *w_value == 0x0302 && payload[1] == SUBCMD_SET_EFFECT {
                    return Some(payload);
                }
            }
            None
        })
        .collect();
    let last_effect = trailing_set_effects
        .last()
        .expect("Expected trailing Set Effect in Music scenario");
    assert_eq!(
        last_effect[2], EFFECT_STARLIGHT_OR_REACTIVE,
        "Unlabeled Frame 553 anomaly must be StarLight"
    );
    let last_period = u16::from_le_bytes([last_effect[3], last_effect[4]]);
    assert_eq!(
        last_period, 300,
        "Unlabeled Frame 553 anomaly must be Fast speed (300ms)"
    );

    // 6. Diagnostic Logging (visible when tests are run with `-- --show-output` or `--nocapture`)
    let tested_ops: usize = scenarios
        .iter()
        .filter(|s| tested_frames.contains(&s.start_frame))
        .map(|s| s.operations.len())
        .sum();
    let excluded_ops: usize = scenarios
        .iter()
        .filter(|s| excluded_frames.contains(&s.start_frame))
        .map(|s| s.operations.len())
        .sum();
    let total_ops = tested_ops + excluded_ops;

    println!("\n[PCAP Conformance Coverage & Audit Summary]");
    println!(
        "  Scenarios:  {} total ({} tested byte-for-byte, {} excluded)",
        scenarios.len(),
        tested_frames.len(),
        excluded_frames.len()
    );
    println!(
        "  Operations: {} total ({} tested, {} excluded [~{} USB wire packets])",
        total_ops,
        tested_ops,
        excluded_ops,
        excluded_ops * 2
    );

    let mut reason_stats: HashMap<ExclusionReason, (usize, usize)> = HashMap::new();
    for s in EXCLUDED_SCENARIOS {
        let ops = scenarios
            .iter()
            .find(|scen| scen.start_frame == s.start_frame)
            .map(|scen| scen.operations.len())
            .unwrap_or(0);
        let entry = reason_stats.entry(s.reason).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += ops;
    }

    println!("  Exclusion Breakdown:");
    for (reason, (scen_count, op_count)) in &reason_stats {
        let label = match reason {
            ExclusionReason::Reapplied => "GUI Redundant Clicks / No-Ops",
            ExclusionReason::RedundantVariantCoveredElsewhere => "Redundant Speed/Palette Variants",
            ExclusionReason::IntermediateCustomizeSweep => "Intermediate Customize Sweeps",
            ExclusionReason::RealtimeAudioStream => "Real-Time Audio Stream (Music)",
            ExclusionReason::AnnotationMismatch => "Known OEM Wire Annotation Mismatch",
        };
        println!(
            "    - {:<36}: {:>2} scenario(s) ({:>4} ops)",
            label, scen_count, op_count
        );
    }
}
