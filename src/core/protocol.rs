use std::time::Duration;

// ============================================================================
// 1. Hardware & USB HID Transport Constants
// ============================================================================

pub const VENDOR_ID: u16 = 0x1462;
pub const PRODUCT_ID: u16 = 0x1565;
pub const INTERFACE_NUMBER: u8 = 0x00;
pub const WINDEX_INTERFACE_0: u16 = 0x0000;

/// Fixed report size for all SET_REPORT payloads and standard GET_REPORT readbacks.
pub const REPORT_LEN: usize = 64;

/// Extended readback length required by GET_REPORT(0x0302) during Music mode.
pub const MUSIC_READBACK_LEN: usize = 65;

/// USB HID Class-Specific bRequest codes.
pub const HID_REQ_GET_REPORT: u8 = 0x01;
pub const HID_REQ_SET_REPORT: u8 = 0x09;
pub const HID_REQ_SET_IDLE: u8 = 0x0a;

/// wValue parameters (ReportType = 0x03 Feature in high byte, ReportID in low byte).
pub const WVALUE_SET_IDLE_ALL: u16 = 0x0000;
pub const WVALUE_REPORT_ID_1: u16 = 0x0301;
pub const WVALUE_REPORT_ID_2: u16 = 0x0302;

/// Report IDs (Byte 0 of 64-byte payload).
pub const REPORT_ID_1: u8 = 0x01;
pub const REPORT_ID_2: u8 = 0x02;

/// Default USB control transfer timeout (must exceed ~256 ms hardware flash write block).
pub const USB_CONTROL_TIMEOUT: Duration = Duration::from_millis(1000);

// ============================================================================
// 2. Command & Subcommand Opcodes (Byte 1)
// ============================================================================

/// Report ID 0x01 opcodes.
pub const CMD_HANDSHAKE_91: u8 = 0x91;
pub const CMD_HANDSHAKE_91_PARAM: u8 = 0x01;
pub const CMD_STATE_QUERY_A0: u8 = 0xa0;
pub const CMD_STATE_QUERY_A0_PARAM: u8 = 0x00;

/// Report ID 0x02 subcommands.
pub const SUBCMD_KEY_MASK: u8 = 0x01;
pub const SUBCMD_SET_EFFECT: u8 = 0x02;
pub const SUBCMD_MUSIC_INIT: u8 = 0x05;
pub const SUBCMD_MUSIC_FRAME: u8 = 0x06;
pub const SUBCMD_FLASH_APPLY: u8 = 0xa0;
pub const SUBCMD_DISCARD: u8 = 0xb0;

// ============================================================================
// 3. Protocol Timing Delays
// ============================================================================

pub const DELAY_STARTUP_SHORT_1: Duration = Duration::from_millis(2);
pub const DELAY_STARTUP_STEP_1: Duration = Duration::from_millis(120);
pub const DELAY_STARTUP_STEP_2: Duration = Duration::from_millis(280);
pub const DELAY_STARTUP_SHORT_2: Duration = Duration::from_millis(4);
pub const DELAY_STARTUP_POST_HANDSHAKE: Duration = Duration::from_millis(145);
pub const DELAY_DUAL_APPLY_EFFECT: Duration = Duration::from_millis(45);

pub const DELAY_PROFILE_DISCARD: Duration = Duration::from_millis(545);
pub const DELAY_PROFILE_PAIR_SHORT: Duration = Duration::from_millis(7);
pub const DELAY_PROFILE_INTER_CYCLE: Duration = Duration::from_millis(60);
pub const DELAY_PROFILE_POST_HANDSHAKE: Duration = Duration::from_millis(30);
pub const DELAY_DANCE_PROFILE_CYCLE: Duration = Duration::from_millis(20);
pub const DELAY_PRE_CLEAR_SHORT: Duration = Duration::from_millis(2);

pub const DELAY_FLASH_APPLY_GLOBAL: Duration = Duration::from_millis(88);
pub const DELAY_FLASH_APPLY_CUSTOMIZE: Duration = Duration::from_millis(640);
pub const DELAY_CLOSE_POST_DISCARD: Duration = Duration::from_millis(55);

pub const DELAY_MUSIC_PRE_INIT: Duration = Duration::from_millis(146);
pub const DELAY_MUSIC_POST_INIT: Duration = Duration::from_millis(717);
pub const DELAY_MUSIC_INTER_FRAME: Duration = Duration::from_micros(8500);
pub const DELAY_MUSIC_EXIT: Duration = Duration::from_millis(115);
pub const DELAY_CUSTOMIZE_INTER_KEY: Duration = Duration::from_millis(1);

// ============================================================================
// 4. Effect IDs & Header Parameter Constants (02 02 Packet Bytes 2..10)
// ============================================================================

pub const EFFECT_DANCE: u8 = 0x00;
pub const EFFECT_STEADY: u8 = 0x01;
pub const EFFECT_BREATH: u8 = 0x02;
pub const EFFECT_COLOR_CYCLE: u8 = 0x03;
pub const EFFECT_WAVE: u8 = 0x04;
pub const EFFECT_STARLIGHT_OR_REACTIVE: u8 = 0x05;
pub const EFFECT_OFF: u8 = 0x06;
pub const EFFECT_FIREWORK_OR_RIPPLE: u8 = 0x07;
pub const EFFECT_LASER_OR_COMET: u8 = 0x08;

/// Little-endian 16-bit period_ms values (Bytes 3..4).
pub const PERIOD_STATIC_DEFAULT: u16 = 65436; // 0xff9c (-100 in i16, wire bytes 9c ff)
pub const PERIOD_OFF_PROFILE_SWITCH: u16 = 100; // 0x0064 (wire bytes 64 00)

pub const PERIOD_BREATH_CYCLE_FAST: u16 = 400; // 0x0190 (wire bytes 90 01)
pub const PERIOD_BREATH_CYCLE_MED: u16 = 800; // 0x0320 (wire bytes 20 03)
pub const PERIOD_BREATH_CYCLE_SLOW: u16 = 1200; // 0x04b0 (wire bytes b0 04)

pub const PERIOD_WAVE_FAST: u16 = 300; // 0x012c (wire bytes 2c 01)
pub const PERIOD_WAVE_MED: u16 = 500; // 0x01f4 (wire bytes f4 01)
pub const PERIOD_WAVE_SLOW: u16 = 800; // 0x0320 (wire bytes 20 03)

pub const PERIOD_DANCE_FAST: u16 = 100; // 0x0064 (wire bytes 64 00)
pub const PERIOD_DANCE_MED: u16 = 200; // 0x00c8 (wire bytes c8 00)
pub const PERIOD_DANCE_SLOW: u16 = 300; // 0x012c (wire bytes 2c 01)

pub const PERIOD_REACTIVE_FAST: u16 = 100; // 0x0064 (wire bytes 64 00)
pub const PERIOD_REACTIVE_MED: u16 = 300; // 0x012c (wire bytes 2c 01)
pub const PERIOD_REACTIVE_SLOW: u16 = 500; // 0x01f4 (wire bytes f4 01)

pub const PERIOD_RIPPLE_FAST: u16 = 200; // 0x00c8 (wire bytes c8 00)
pub const PERIOD_RIPPLE_MED: u16 = 300; // 0x012c (wire bytes 2c 01)
pub const PERIOD_RIPPLE_SLOW: u16 = 400; // 0x0190 (wire bytes 90 01)

pub const PERIOD_COMET_FAST: u16 = 80; // 0x0050 (wire bytes 50 00)
pub const PERIOD_COMET_MED: u16 = 170; // 0x00aa (wire bytes aa 00)
pub const PERIOD_COMET_SLOW: u16 = 229; // 0x00e5 (wire bytes e5 00)

pub const PERIOD_STARLIGHT_FIXED: u16 = 300; // 0x012c (wire bytes 2c 01)
pub const PERIOD_FIREWORK_FIXED: u16 = 400; // 0x0190 (wire bytes 90 01)
pub const PERIOD_LASER_FIXED: u16 = 130; // 0x0082 (wire bytes 82 00)

pub const PERIOD_CUSTOMIZE_STEADY_FAST: u16 = 100; // 0x0064
pub const PERIOD_CUSTOMIZE_STEADY_ALT: u16 = 300; // 0x012c

/// Direction coordinates (Byte 5 = direction_x, Byte 6 = direction_y).
pub const DIR_X_RIGHT_TO_LEFT: u8 = 0x00;
pub const DIR_X_ZERO: u8 = 0x00;
pub const DIR_X_CENTER: u8 = 0x32;
pub const DIR_X_LEFT_TO_RIGHT: u8 = 0x64;

pub const DIR_Y_DEFAULT: u8 = 0x32;
pub const DIR_Y_ZERO: u8 = 0x00;

/// Density / tail spread parameters (Byte 7).
pub const DENSITY_DEFAULT: u8 = 0x0a; // 10
pub const DENSITY_FIREWORK: u8 = 0x1e; // 30
pub const DENSITY_HIGH: u8 = 0x50; // 80

/// Secondary speed step values (Byte 8).
pub const STEP_CUSTOMIZE_ZERO: u8 = 0x00;
pub const STEP_DEFAULT_GLOBAL: u8 = 0x01;
pub const STEP_REACTIVE_DEFAULT: u8 = 0x02;

pub const STEP_STARLIGHT_FAST: u8 = 0x01;
pub const STEP_STARLIGHT_MED: u8 = 0x03;
pub const STEP_STARLIGHT_SLOW: u8 = 0x05;

pub const STEP_FIREWORK_FAST: u8 = 0x07;
pub const STEP_FIREWORK_MED: u8 = 0x0a;
pub const STEP_FIREWORK_SLOW: u8 = 0x0d;

pub const STEP_LASER_FAST: u8 = 0x02;
pub const STEP_LASER_MED: u8 = 0x04;
pub const STEP_LASER_SLOW: u8 = 0x06;

/// Sub-mode flag (Byte 9): 0x01 = autonomous continuous, 0x00 = reactive or standard.
pub const SUBMODE_REACTIVE_OR_STANDARD: u8 = 0x00;
pub const SUBMODE_AUTONOMOUS: u8 = 0x01;

// ============================================================================
// 5. Color Stop Positions & Predefined Palettes (Bytes 11..38)
// ============================================================================

pub const POS_WRAP_0: u8 = 0x00;
pub const POS_50_PERCENT: u8 = 0x32;
pub const POS_100_PERCENT: u8 = 0x64;

pub const POS_3COLOR_1: u8 = 0x21; // 33%
pub const POS_3COLOR_2: u8 = 0x42; // 66%
pub const POS_3COLOR_3: u8 = 0x64; // 100%

/// Template 4: 6-Color Rainbow Loop + Red wrap stop (28 bytes at Bytes 11..38)
/// Used by Color Cycle and Wave.
pub const RAINBOW_LOOP_STOPS: [u8; 28] = [
    0xff, 0x00, 0x00, 0x10, // Red @ 16%
    0xff, 0xff, 0x00, 0x20, // Yellow @ 32%
    0x00, 0xff, 0x00, 0x30, // Green @ 48%
    0x00, 0xff, 0xff, 0x40, // Cyan @ 64%
    0x00, 0x00, 0xff, 0x50, // Blue @ 80%
    0xff, 0x00, 0xff, 0x64, // Magenta @ 100%
    0xff, 0x00, 0x00, 0x00, // Red wrap @ 0%
];

/// Template 5B: Black Lead-In Rainbow (28 bytes at Bytes 11..38)
/// Used by StarLight, Firework, Laser, Reactive, Ripple, and Comet.
pub const BLACK_LEAD_IN_RAINBOW_STOPS: [u8; 28] = [
    0x00, 0x00, 0x00, 0x0e, // Black @ 14%
    0xff, 0x00, 0x00, 0x1c, // Red @ 28%
    0xff, 0xff, 0x00, 0x2a, // Yellow @ 42%
    0x00, 0xff, 0x00, 0x38, // Green @ 56%
    0x00, 0xff, 0xff, 0x46, // Cyan @ 70%
    0x00, 0x00, 0xff, 0x54, // Blue @ 84%
    0xff, 0x00, 0xff, 0x64, // Magenta @ 100%
];

/// Default Music Mode VU-Meter initialization palette (Bytes 5..20 of 02 05):
/// Green, Green, Yellow, Yellow, Red, followed by 0xff terminator.
pub const MUSIC_DEFAULT_PALETTE_BYTES: [u8; 16] = [
    0x00, 0xff, 0x00, // Stop 1: Green
    0x00, 0xff, 0x00, // Stop 2: Green
    0xff, 0xff, 0x00, // Stop 3: Yellow
    0xff, 0xff, 0x00, // Stop 4: Yellow
    0xff, 0x00, 0x00, // Stop 5: Red
    0xff,             // Byte 20: Terminator / Brightness scale
];

// ============================================================================
// 6. Per-Key Bitmask Constants (02 01 Bytes 2..33)
// ============================================================================

/// Global key mask selecting all 256 bit positions (Bytes 2..33 = 0xff).
pub const GLOBAL_KEY_MASK: [u8; 32] = [0xff; 32];

/// Bitwise OR union of all 98 valid RGB keys on the MSI Creator A16 AI+ keyboard.
pub const VALID_98_KEY_MASK: [u8; 32] = [
    0xf0, 0xff, 0xff, 0xff, 0xff, 0xef, 0xfb, 0xff,
    0x7f, 0x92, 0xff, 0xff, 0x1f, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x7f, 0x00, 0x03, 0x00,
];

/// Confirmed physical key coordinate for the `Esc` key:
/// Packet Byte Offset = 7 (Mask Byte 5), Bitmask = 0x02 (Bit 41).
pub const KEY_ESC_COORD: (usize, u8) = (7, 0x02);

/// Official 98-key Customize serialization order as (Packet_Byte_Offset, Bitmask_Hex),
/// where Packet_Byte_Offset is in 2..=33 of the 02 01 report.
pub const SERIALIZATION_ORDER_98_KEYS: [(usize, u8); 98] = [
    // Group 1 (16 keys)
    (7, 0x02),  (9, 0x04),  (9, 0x08),  (9, 0x10),  (9, 0x20),  (9, 0x40),  (9, 0x80),  (10, 0x01),
    (10, 0x02), (10, 0x04), (10, 0x08), (10, 0x10), (10, 0x20), (10, 0x40), (11, 0x02), (11, 0x10),
    // Group 2 (10 keys)
    (12, 0x20), (12, 0x10), (32, 0x02), (12, 0x04), (12, 0x01), (12, 0x02), (11, 0x80), (12, 0x80),
    (12, 0x40), (12, 0x08),
    // Group 3 (12 keys)
    (13, 0x80), (14, 0x01), (14, 0x02), (13, 0x10), (13, 0x20), (13, 0x40), (13, 0x02), (13, 0x04),
    (13, 0x08), (14, 0x04), (14, 0x08), (13, 0x01),
    // Group 4 (15 keys)
    (8, 0x20),  (5, 0x40),  (5, 0x80),  (6, 0x01),  (6, 0x02),  (6, 0x04),  (6, 0x08),  (6, 0x10),
    (6, 0x20),  (6, 0x40),  (6, 0x80),  (7, 0x20),  (7, 0x40),  (7, 0x04),  (7, 0x08),
    // Group 5 (14 keys)
    (4, 0x10),  (5, 0x04),  (3, 0x01),  (4, 0x20),  (4, 0x80),  (5, 0x10),  (5, 0x01),  (3, 0x10),
    (4, 0x04),  (4, 0x08),  (7, 0x80),  (8, 0x01),  (8, 0x02),  (9, 0x02),
    // Group 6 (13 keys)
    (2, 0x10),  (4, 0x40),  (2, 0x80),  (3, 0x02),  (3, 0x04),  (3, 0x08),  (3, 0x20),  (3, 0x40),
    (3, 0x80),  (8, 0x08),  (8, 0x10),  (7, 0x01),  (30, 0x02),
    // Group 7 (10 keys)
    (5, 0x20),  (5, 0x08),  (2, 0x40),  (5, 0x02),  (2, 0x20),  (4, 0x02),  (4, 0x01),  (8, 0x40),
    (8, 0x80),  (9, 0x01),
    // Group 8 (8 keys)
    (30, 0x20), (30, 0x01), (30, 0x08), (30, 0x04), (30, 0x40), (14, 0x10), (30, 0x10), (32, 0x01),
];
