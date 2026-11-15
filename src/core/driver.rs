use super::protocol::*;
use super::transport::{HidControlTransport, TransportError};

/// RGB color tuple `(R, G, B)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const WHITE: Self = Self(255, 255, 255);
    pub const RED: Self = Self(255, 0, 0);
    pub const GREEN: Self = Self(0, 255, 0);
    pub const BLUE: Self = Self(0, 0, 255);
    pub const CYAN: Self = Self(0, 255, 255);
    pub const LIGHT_CYAN: Self = Self(128, 255, 255);
    pub const YELLOW: Self = Self(255, 255, 0);
    pub const MAGENTA: Self = Self(255, 0, 255);
    pub const ORANGE: Self = Self(255, 185, 0);
    pub const WARM_ORANGE: Self = Self(255, 185, 15);
}

/// Animation speed preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speed {
    Slow,
    Medium,
    Fast,
}

/// Wave propagation direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveDirection {
    LeftToRight,
    RightToLeft,
}

/// Palette choice for Color Cycle and Wave modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CyclePalette {
    Rainbow,
    ThreeColor(Rgb, Rgb, Rgb),
}

/// Palette choice for StarLight, Firework, Laser, Reactive, Ripple, and Comet modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransientPalette {
    Rainbow,
    SingleColor(Rgb),
}

/// Target profile configuration used when executing a profile switch sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileTargetMode {
    Off,
    Dance { speed: Speed },
    StandardEffect([u8; REPORT_LEN]),
}

/// Per-key effect specification for Customize mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerKeyEffect {
    Steady(Rgb),
    Breath { fg: Rgb, bg: Rgb, speed: Speed },
    ColorCycle { palette: CyclePalette, speed: Speed },
    Wave { palette: CyclePalette, speed: Speed },
    Dance { speed: Speed },
    Reactive { color: Rgb, speed: Speed },
    Ripple { palette: TransientPalette, speed: Speed },
    Disabled,
}

/// A single per-key or key-group assignment in Customize mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyGroupAssignment {
    /// 32-byte key selection bitmask (corresponds to Bytes 2..33 of `02 01`).
    pub mask: [u8; 32],
    pub effect: PerKeyEffect,
}

impl KeyGroupAssignment {
    /// Helper to create a single-key assignment from a `(Packet_Byte_Offset, Bitmask_Hex)` coordinate,
    /// where `packet_byte_offset` is in `2..=33`.
    pub fn single_key(coord: (usize, u8), effect: PerKeyEffect) -> Self {
        let mut mask = [0u8; 32];
        let (pkt_offset, bit) = coord;
        assert!(
            (2..=33).contains(&pkt_offset),
            "Packet byte offset must be in 2..=33"
        );
        mask[pkt_offset - 2] = bit;
        Self { mask, effect }
    }
}

/// High-level driver for the MSI Per-Key RGB Keyboard Controller (`0x1462:0x1565`).
pub struct MsiKeyboardDriver<T: HidControlTransport> {
    transport: T,
}

impl<T: HidControlTransport> MsiKeyboardDriver<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub fn into_inner(self) -> T {
        self.transport
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    // ========================================================================
    // Low-Level Lockstep Command Helpers
    // ========================================================================

    /// Sends a 64-byte `SET_REPORT` paired in strict lockstep with a 64-byte `GET_REPORT(0x0301)`.
    pub fn send_command_lockstep(
        &mut self,
        w_value: u16,
        payload: &[u8; REPORT_LEN],
    ) -> Result<Vec<u8>, TransportError> {
        self.transport.set_report(w_value, payload)?;
        self.transport
            .get_report(WVALUE_REPORT_ID_1, REPORT_LEN as u16)
    }

    /// Sends a 64-byte Music `SET_REPORT(0x0302)` paired with a 65-byte `GET_REPORT(0x0302)`.
    pub fn send_music_command_lockstep(
        &mut self,
        payload: &[u8; REPORT_LEN],
    ) -> Result<Vec<u8>, TransportError> {
        self.transport.set_report(WVALUE_REPORT_ID_2, payload)?;
        self.transport
            .get_report(WVALUE_REPORT_ID_2, MUSIC_READBACK_LEN as u16)
    }

    /// Sends a paired `[02 01 (Key Mask) + 02 02 (Effect Config)]` sequence,
    /// each followed by its mandatory `GET_REPORT(0x0301)` readback.
    pub fn send_lighting_pair(
        &mut self,
        key_mask: &[u8; 32],
        effect_payload: &[u8; REPORT_LEN],
    ) -> Result<(), TransportError> {
        let mask_pkt = build_key_mask_packet(key_mask);
        self.send_command_lockstep(WVALUE_REPORT_ID_2, &mask_pkt)?;
        self.send_command_lockstep(WVALUE_REPORT_ID_2, effect_payload)?;
        Ok(())
    }

    /// Sends the mandatory Pre-Effect Global Clear (`02 01 ff*32` + `02 02 06 00...`).
    pub fn send_pre_effect_global_clear(&mut self) -> Result<(), TransportError> {
        let clear_pkt = build_off_clear_packet();
        self.send_lighting_pair(&GLOBAL_KEY_MASK, &clear_pkt)
    }

    // ========================================================================
    // Session Lifecycle & Persistence Operations
    // ========================================================================

    /// Executes the full Application Startup handshake sequence and applies the initial effect twice.
    pub fn initialize_session(
        &mut self,
        send_set_idle: bool,
        initial_effect_payload: &[u8; REPORT_LEN],
    ) -> Result<(), TransportError> {
        if send_set_idle {
            self.transport.set_idle()?;
        }
        let hs_91 = build_handshake_91_packet();
        let sq_a0 = build_state_query_a0_packet();

        self.send_command_lockstep(WVALUE_REPORT_ID_1, &hs_91)?;
        self.transport.sleep(DELAY_STARTUP_SHORT_1);
        self.send_command_lockstep(WVALUE_REPORT_ID_1, &sq_a0)?;
        self.transport.sleep(DELAY_STARTUP_STEP_1);

        self.send_command_lockstep(WVALUE_REPORT_ID_1, &hs_91)?;
        self.transport.sleep(DELAY_STARTUP_STEP_2);

        self.send_command_lockstep(WVALUE_REPORT_ID_1, &sq_a0)?;
        self.transport.sleep(DELAY_STARTUP_SHORT_2);
        self.send_command_lockstep(WVALUE_REPORT_ID_1, &hs_91)?;
        self.transport.sleep(DELAY_STARTUP_POST_HANDSHAKE);

        self.send_lighting_pair(&GLOBAL_KEY_MASK, initial_effect_payload)?;
        self.transport.sleep(DELAY_DUAL_APPLY_EFFECT);
        self.send_lighting_pair(&GLOBAL_KEY_MASK, initial_effect_payload)?;
        Ok(())
    }

    /// Executes the Profile Switch handshake sequence.
    pub fn switch_profile(
        &mut self,
        discard_unsaved_first: bool,
        target: ProfileTargetMode,
    ) -> Result<(), TransportError> {
        if discard_unsaved_first {
            let discard_pkt = build_discard_packet();
            self.send_command_lockstep(WVALUE_REPORT_ID_2, &discard_pkt)?;
            self.transport.sleep(DELAY_PROFILE_DISCARD);
        }

        let hs_91 = build_handshake_91_packet();
        let sq_a0 = build_state_query_a0_packet();

        self.send_command_lockstep(WVALUE_REPORT_ID_1, &sq_a0)?;
        self.transport.sleep(DELAY_PROFILE_PAIR_SHORT);
        self.send_command_lockstep(WVALUE_REPORT_ID_1, &hs_91)?;
        self.transport.sleep(DELAY_PROFILE_INTER_CYCLE);

        self.send_command_lockstep(WVALUE_REPORT_ID_1, &sq_a0)?;
        self.transport.sleep(DELAY_PROFILE_PAIR_SHORT);
        self.send_command_lockstep(WVALUE_REPORT_ID_1, &hs_91)?;
        self.transport.sleep(DELAY_PROFILE_POST_HANDSHAKE);

        match target {
            ProfileTargetMode::Off => {
                let off_pkt = build_off_profile_switch_packet();
                self.send_lighting_pair(&GLOBAL_KEY_MASK, &off_pkt)?;
                self.transport.sleep(DELAY_DUAL_APPLY_EFFECT);
                self.send_lighting_pair(&GLOBAL_KEY_MASK, &off_pkt)?;
            }
            ProfileTargetMode::StandardEffect(eff_pkt) => {
                self.send_lighting_pair(&GLOBAL_KEY_MASK, &eff_pkt)?;
                self.transport.sleep(DELAY_DUAL_APPLY_EFFECT);
                self.send_lighting_pair(&GLOBAL_KEY_MASK, &eff_pkt)?;
            }
            ProfileTargetMode::Dance { speed } => {
                let dance_pkt = build_dance_packet(speed, DIR_X_CENTER);
                // Cycle 1
                self.send_pre_effect_global_clear()?;
                self.transport.sleep(DELAY_PRE_CLEAR_SHORT);
                self.send_lighting_pair(&GLOBAL_KEY_MASK, &dance_pkt)?;
                self.transport.sleep(DELAY_DANCE_PROFILE_CYCLE);
                // Cycle 2
                self.send_pre_effect_global_clear()?;
                self.transport.sleep(DELAY_PRE_CLEAR_SHORT);
                self.send_lighting_pair(&GLOBAL_KEY_MASK, &dance_pkt)?;
            }
        }
        Ok(())
    }

    /// Commits the current lighting configuration to non-volatile flash memory (`02 a0` sent twice).
    pub fn apply_to_flash(&mut self, is_customize_layout: bool) -> Result<(), TransportError> {
        let apply_pkt = build_flash_apply_packet();
        self.send_command_lockstep(WVALUE_REPORT_ID_2, &apply_pkt)?;

        let pause = if is_customize_layout {
            DELAY_FLASH_APPLY_CUSTOMIZE
        } else {
            DELAY_FLASH_APPLY_GLOBAL
        };
        self.transport.sleep(pause);

        self.send_command_lockstep(WVALUE_REPORT_ID_2, &apply_pkt)?;
        Ok(())
    }

    /// Discards unsaved preview changes (`02 b0`) and turns off the keyboard before application exit.
    pub fn discard_and_close(&mut self) -> Result<(), TransportError> {
        let discard_pkt = build_discard_packet();
        self.send_command_lockstep(WVALUE_REPORT_ID_2, &discard_pkt)?;
        self.transport.sleep(DELAY_CLOSE_POST_DISCARD);
        self.set_off()
    }

    // ========================================================================
    // High-Level Global Lighting Mode Commands
    // ========================================================================

    /// Turns off all keyboard LEDs (`effect_id = 0x06`).
    pub fn set_off(&mut self) -> Result<(), TransportError> {
        let pkt = build_off_standard_packet();
        self.send_lighting_pair(&GLOBAL_KEY_MASK, &pkt)
    }

    /// Configures Global **Steady** mode (`effect_id = 0x01`) with a static RGB color.
    pub fn set_steady(&mut self, color: Rgb) -> Result<(), TransportError> {
        let pkt = build_steady_packet(color);
        self.send_lighting_pair(&GLOBAL_KEY_MASK, &pkt)
    }

    /// Configures Global **Breath** mode (`effect_id = 0x02`) with Foreground color, Background color, and Speed.
    pub fn set_breath(&mut self, fg: Rgb, bg: Rgb, speed: Speed) -> Result<(), TransportError> {
        let pkt = build_breath_packet(fg, bg, speed);
        self.send_lighting_pair(&GLOBAL_KEY_MASK, &pkt)
    }

    /// Configures Global **Color Cycle** mode (`effect_id = 0x03`).
    pub fn set_color_cycle(
        &mut self,
        palette: CyclePalette,
        speed: Speed,
    ) -> Result<(), TransportError> {
        let pkt = build_color_cycle_packet(palette, speed);
        self.send_lighting_pair(&GLOBAL_KEY_MASK, &pkt)
    }

    /// Configures Global **Wave** mode (`effect_id = 0x04`).
    pub fn set_wave(
        &mut self,
        palette: CyclePalette,
        speed: Speed,
        direction: WaveDirection,
    ) -> Result<(), TransportError> {
        let pkt = build_wave_packet(palette, speed, direction);
        self.send_lighting_pair(&GLOBAL_KEY_MASK, &pkt)
    }

    /// Configures Global **Dance** mode (`effect_id = 0x00`), preceded by the mandatory all-zero Off clear.
    pub fn set_dance(&mut self, speed: Speed) -> Result<(), TransportError> {
        self.send_pre_effect_global_clear()?;
        self.transport.sleep(DELAY_PRE_CLEAR_SHORT);
        let pkt = build_dance_packet(speed, DIR_X_ZERO);
        self.send_lighting_pair(&GLOBAL_KEY_MASK, &pkt)
    }

    /// Configures Global **StarLight** mode (`effect_id = 0x05`, `sub_mode_flag = 0x01`).
    pub fn set_starlight(
        &mut self,
        palette: TransientPalette,
        speed: Speed,
    ) -> Result<(), TransportError> {
        let pkt = build_starlight_packet(palette, speed);
        self.send_lighting_pair(&GLOBAL_KEY_MASK, &pkt)
    }

    /// Configures Global **Firework** mode (`effect_id = 0x07`, `sub_mode_flag = 0x01`).
    pub fn set_firework(
        &mut self,
        palette: TransientPalette,
        speed: Speed,
    ) -> Result<(), TransportError> {
        let pkt = build_firework_packet(palette, speed);
        self.send_lighting_pair(&GLOBAL_KEY_MASK, &pkt)
    }

    /// Configures Global **Laser** mode (`effect_id = 0x08`, `sub_mode_flag = 0x01`).
    pub fn set_laser(
        &mut self,
        palette: TransientPalette,
        speed: Speed,
    ) -> Result<(), TransportError> {
        let pkt = build_laser_packet(palette, speed);
        self.send_lighting_pair(&GLOBAL_KEY_MASK, &pkt)
    }

    /// Configures Global **Reactive** mode (`effect_id = 0x05`, `sub_mode_flag = 0x00`).
    pub fn set_reactive(
        &mut self,
        palette: TransientPalette,
        speed: Speed,
    ) -> Result<(), TransportError> {
        let pkt = build_reactive_packet(palette, speed);
        self.send_lighting_pair(&GLOBAL_KEY_MASK, &pkt)
    }

    /// Configures Global **Ripple** mode (`effect_id = 0x07`, `sub_mode_flag = 0x00`).
    pub fn set_ripple(
        &mut self,
        palette: TransientPalette,
        speed: Speed,
    ) -> Result<(), TransportError> {
        let pkt = build_ripple_packet(palette, speed);
        self.send_lighting_pair(&GLOBAL_KEY_MASK, &pkt)
    }

    /// Configures Global **Comet** mode (`effect_id = 0x08`, `sub_mode_flag = 0x00`).
    pub fn set_comet(
        &mut self,
        palette: TransientPalette,
        speed: Speed,
    ) -> Result<(), TransportError> {
        let pkt = build_comet_packet(palette, speed);
        self.send_lighting_pair(&GLOBAL_KEY_MASK, &pkt)
    }

    // ========================================================================
    // Per-Key (Customize) Mode Operations
    // ========================================================================

    /// Writes a sequence of per-key or grouped-key effect assignments in Customize mode.
    /// Set `clear_first = true` when initially entering Customize mode or loading a new full preset;
    /// set `clear_first = false` for incremental per-key updates.
    pub fn set_customize_layout(
        &mut self,
        clear_first: bool,
        assignments: &[KeyGroupAssignment],
    ) -> Result<(), TransportError> {
        if clear_first {
            self.send_pre_effect_global_clear()?;
            self.transport.sleep(DELAY_PRE_CLEAR_SHORT);
        }

        for (idx, item) in assignments.iter().enumerate() {
            let eff_pkt = build_per_key_effect_packet(item.effect);
            self.send_lighting_pair(&item.mask, &eff_pkt)?;
            if idx + 1 < assignments.len() {
                self.transport.sleep(DELAY_CUSTOMIZE_INTER_KEY);
            }
        }
        Ok(())
    }

    /// Serializes a full 98-key Customize layout in the exact 8-group hardware transmission order,
    /// calling `effect_for_key(coord)` for each of the 98 valid `(Packet_Byte_Offset, Bitmask_Hex)` keys.
    pub fn set_full_98_key_customize<F>(
        &mut self,
        clear_first: bool,
        mut effect_for_key: F,
    ) -> Result<(), TransportError>
    where
        F: FnMut((usize, u8)) -> PerKeyEffect,
    {
        let assignments: Vec<KeyGroupAssignment> = SERIALIZATION_ORDER_98_KEYS
            .iter()
            .map(|&coord| KeyGroupAssignment::single_key(coord, effect_for_key(coord)))
            .collect();

        self.set_customize_layout(clear_first, &assignments)
    }

    // ========================================================================
    // Real-Time Music Mode (02 05 & 02 06)
    // ========================================================================

    /// Initializes Music Mode: sends the mandatory global Off clear, waits 146 ms,
    /// sends the `02 05` VU-meter color palette (with 65B readback), and waits 717 ms.
    pub fn start_music_mode(&mut self) -> Result<(), TransportError> {
        self.send_pre_effect_global_clear()?;
        self.transport.sleep(DELAY_MUSIC_PRE_INIT);

        let init_pkt = build_music_init_packet(&MUSIC_DEFAULT_PALETTE_BYTES);
        self.send_music_command_lockstep(&init_pkt)?;
        self.transport.sleep(DELAY_MUSIC_POST_INIT);
        Ok(())
    }

    /// Streams a single 62-byte real-time audio visualization frame (`02 06`) and performs
    /// its mandatory 65-byte `GET_REPORT(0x0302)` readback, followed by an ~8.5 ms pacing sleep.
    pub fn send_music_frame(&mut self, audio_data: &[u8; 62]) -> Result<(), TransportError> {
        let frame_pkt = build_music_frame_packet(audio_data);
        self.send_music_command_lockstep(&frame_pkt)?;
        self.transport.sleep(DELAY_MUSIC_INTER_FRAME);
        Ok(())
    }
}

// ============================================================================
// Pure Packet Builder Functions (64-Byte HID Feature Reports)
// ============================================================================

pub fn build_handshake_91_packet() -> [u8; REPORT_LEN] {
    let mut pkt = [0u8; REPORT_LEN];
    pkt[0] = REPORT_ID_1;
    pkt[1] = CMD_HANDSHAKE_91;
    pkt[2] = CMD_HANDSHAKE_91_PARAM;
    pkt
}

pub fn build_state_query_a0_packet() -> [u8; REPORT_LEN] {
    let mut pkt = [0u8; REPORT_LEN];
    pkt[0] = REPORT_ID_1;
    pkt[1] = CMD_STATE_QUERY_A0;
    pkt[2] = CMD_STATE_QUERY_A0_PARAM;
    pkt
}

pub fn build_key_mask_packet(mask: &[u8; 32]) -> [u8; REPORT_LEN] {
    let mut pkt = [0u8; REPORT_LEN];
    pkt[0] = REPORT_ID_2;
    pkt[1] = SUBCMD_KEY_MASK;
    pkt[2..34].copy_from_slice(mask);
    pkt
}

pub fn build_flash_apply_packet() -> [u8; REPORT_LEN] {
    let mut pkt = [0u8; REPORT_LEN];
    pkt[0] = REPORT_ID_2;
    pkt[1] = SUBCMD_FLASH_APPLY;
    pkt
}

pub fn build_discard_packet() -> [u8; REPORT_LEN] {
    let mut pkt = [0u8; REPORT_LEN];
    pkt[0] = REPORT_ID_2;
    pkt[1] = SUBCMD_DISCARD;
    pkt
}

pub fn build_music_init_packet(palette_and_term: &[u8; 16]) -> [u8; REPORT_LEN] {
    let mut pkt = [0u8; REPORT_LEN];
    pkt[0] = REPORT_ID_2;
    pkt[1] = SUBCMD_MUSIC_INIT;
    // Bytes 2..4 are 0x00, Bytes 5..20 hold 5 RGB stops + 0xff terminator.
    pkt[5..21].copy_from_slice(palette_and_term);
    pkt
}

pub fn build_music_frame_packet(audio_data: &[u8; 62]) -> [u8; REPORT_LEN] {
    let mut pkt = [0u8; REPORT_LEN];
    pkt[0] = REPORT_ID_2;
    pkt[1] = SUBCMD_MUSIC_FRAME;
    pkt[2..64].copy_from_slice(audio_data);
    pkt
}

#[allow(clippy::too_many_arguments)]
pub fn build_effect_header(
    effect_id: u8,
    period_ms: u16,
    dir_x: u8,
    dir_y: u8,
    density: u8,
    speed_step: u8,
    sub_mode: u8,
) -> [u8; REPORT_LEN] {
    let mut pkt = [0u8; REPORT_LEN];
    let period_le = period_ms.to_le_bytes();
    pkt[0] = REPORT_ID_2;
    pkt[1] = SUBCMD_SET_EFFECT;
    pkt[2] = effect_id;
    pkt[3] = period_le[0];
    pkt[4] = period_le[1];
    pkt[5] = dir_x;
    pkt[6] = dir_y;
    pkt[7] = density;
    pkt[8] = speed_step;
    pkt[9] = sub_mode;
    pkt[10] = 0x00;
    pkt
}

pub fn build_off_standard_packet() -> [u8; REPORT_LEN] {
    build_effect_header(
        EFFECT_OFF,
        PERIOD_STATIC_DEFAULT,
        DIR_X_CENTER,
        DIR_Y_DEFAULT,
        DENSITY_DEFAULT,
        STEP_DEFAULT_GLOBAL,
        SUBMODE_REACTIVE_OR_STANDARD,
    )
}

pub fn build_off_profile_switch_packet() -> [u8; REPORT_LEN] {
    build_effect_header(
        EFFECT_OFF,
        PERIOD_OFF_PROFILE_SWITCH,
        DIR_X_CENTER,
        DIR_Y_DEFAULT,
        DENSITY_DEFAULT,
        STEP_DEFAULT_GLOBAL,
        SUBMODE_REACTIVE_OR_STANDARD,
    )
}

pub fn build_off_clear_packet() -> [u8; REPORT_LEN] {
    build_effect_header(
        EFFECT_OFF,
        0,
        DIR_X_ZERO,
        DIR_Y_ZERO,
        0x00,
        0x00,
        SUBMODE_REACTIVE_OR_STANDARD,
    )
}

pub fn build_steady_packet(color: Rgb) -> [u8; REPORT_LEN] {
    let mut pkt = build_effect_header(
        EFFECT_STEADY,
        PERIOD_STATIC_DEFAULT,
        DIR_X_CENTER,
        DIR_Y_DEFAULT,
        DENSITY_DEFAULT,
        STEP_DEFAULT_GLOBAL,
        SUBMODE_REACTIVE_OR_STANDARD,
    );
    write_template_1_steady(&mut pkt, color);
    pkt
}

pub fn build_breath_packet(fg: Rgb, bg: Rgb, speed: Speed) -> [u8; REPORT_LEN] {
    let period = match speed {
        Speed::Slow => PERIOD_BREATH_CYCLE_SLOW,
        Speed::Medium => PERIOD_BREATH_CYCLE_MED,
        Speed::Fast => PERIOD_BREATH_CYCLE_FAST,
    };
    let mut pkt = build_effect_header(
        EFFECT_BREATH,
        period,
        DIR_X_CENTER,
        DIR_Y_DEFAULT,
        DENSITY_DEFAULT,
        STEP_DEFAULT_GLOBAL,
        SUBMODE_REACTIVE_OR_STANDARD,
    );
    write_template_2_breath(&mut pkt, fg, bg);
    pkt
}

pub fn build_color_cycle_packet(palette: CyclePalette, speed: Speed) -> [u8; REPORT_LEN] {
    let period = match speed {
        Speed::Slow => PERIOD_BREATH_CYCLE_SLOW,
        Speed::Medium => PERIOD_BREATH_CYCLE_MED,
        Speed::Fast => PERIOD_BREATH_CYCLE_FAST,
    };
    let mut pkt = build_effect_header(
        EFFECT_COLOR_CYCLE,
        period,
        DIR_X_CENTER,
        DIR_Y_DEFAULT,
        DENSITY_DEFAULT,
        STEP_DEFAULT_GLOBAL,
        SUBMODE_REACTIVE_OR_STANDARD,
    );
    write_cycle_palette(&mut pkt, palette);
    pkt
}

pub fn build_wave_packet(
    palette: CyclePalette,
    speed: Speed,
    direction: WaveDirection,
) -> [u8; REPORT_LEN] {
    let period = match speed {
        Speed::Slow => PERIOD_WAVE_SLOW,
        Speed::Medium => PERIOD_WAVE_MED,
        Speed::Fast => PERIOD_WAVE_FAST,
    };
    let dir_x = match direction {
        WaveDirection::LeftToRight => DIR_X_LEFT_TO_RIGHT,
        WaveDirection::RightToLeft => DIR_X_RIGHT_TO_LEFT,
    };
    let mut pkt = build_effect_header(
        EFFECT_WAVE,
        period,
        dir_x,
        DIR_Y_DEFAULT,
        DENSITY_DEFAULT,
        STEP_DEFAULT_GLOBAL,
        SUBMODE_REACTIVE_OR_STANDARD,
    );
    write_cycle_palette(&mut pkt, palette);
    pkt
}

pub fn build_dance_packet(speed: Speed, dir_x: u8) -> [u8; REPORT_LEN] {
    let period = match speed {
        Speed::Slow => PERIOD_DANCE_SLOW,
        Speed::Medium => PERIOD_DANCE_MED,
        Speed::Fast => PERIOD_DANCE_FAST,
    };
    build_effect_header(
        EFFECT_DANCE,
        period,
        dir_x,
        DIR_Y_DEFAULT,
        DENSITY_DEFAULT,
        STEP_DEFAULT_GLOBAL,
        SUBMODE_REACTIVE_OR_STANDARD,
    )
}

pub fn build_starlight_packet(palette: TransientPalette, speed: Speed) -> [u8; REPORT_LEN] {
    let step = match speed {
        Speed::Slow => STEP_STARLIGHT_SLOW,
        Speed::Medium => STEP_STARLIGHT_MED,
        Speed::Fast => STEP_STARLIGHT_FAST,
    };
    let mut pkt = build_effect_header(
        EFFECT_STARLIGHT_OR_REACTIVE,
        PERIOD_STARLIGHT_FIXED,
        DIR_X_ZERO,
        DIR_Y_DEFAULT,
        DENSITY_DEFAULT,
        step,
        SUBMODE_AUTONOMOUS,
    );
    write_transient_palette(&mut pkt, palette);
    pkt
}

pub fn build_firework_packet(palette: TransientPalette, speed: Speed) -> [u8; REPORT_LEN] {
    let step = match speed {
        Speed::Slow => STEP_FIREWORK_SLOW,
        Speed::Medium => STEP_FIREWORK_MED,
        Speed::Fast => STEP_FIREWORK_FAST,
    };
    let mut pkt = build_effect_header(
        EFFECT_FIREWORK_OR_RIPPLE,
        PERIOD_FIREWORK_FIXED,
        DIR_X_ZERO,
        DIR_Y_DEFAULT,
        DENSITY_FIREWORK,
        step,
        SUBMODE_AUTONOMOUS,
    );
    write_transient_palette(&mut pkt, palette);
    pkt
}

pub fn build_laser_packet(palette: TransientPalette, speed: Speed) -> [u8; REPORT_LEN] {
    let step = match speed {
        Speed::Slow => STEP_LASER_SLOW,
        Speed::Medium => STEP_LASER_MED,
        Speed::Fast => STEP_LASER_FAST,
    };
    let mut pkt = build_effect_header(
        EFFECT_LASER_OR_COMET,
        PERIOD_LASER_FIXED,
        DIR_X_ZERO,
        DIR_Y_DEFAULT,
        DENSITY_HIGH,
        step,
        SUBMODE_AUTONOMOUS,
    );
    write_transient_palette(&mut pkt, palette);
    pkt
}

pub fn build_reactive_packet(palette: TransientPalette, speed: Speed) -> [u8; REPORT_LEN] {
    let period = match speed {
        Speed::Slow => PERIOD_REACTIVE_SLOW,
        Speed::Medium => PERIOD_REACTIVE_MED,
        Speed::Fast => PERIOD_REACTIVE_FAST,
    };
    let mut pkt = build_effect_header(
        EFFECT_STARLIGHT_OR_REACTIVE,
        period,
        DIR_X_ZERO,
        DIR_Y_DEFAULT,
        DENSITY_HIGH,
        STEP_REACTIVE_DEFAULT,
        SUBMODE_REACTIVE_OR_STANDARD,
    );
    write_transient_palette(&mut pkt, palette);
    pkt
}

pub fn build_ripple_packet(palette: TransientPalette, speed: Speed) -> [u8; REPORT_LEN] {
    let period = match speed {
        Speed::Slow => PERIOD_RIPPLE_SLOW,
        Speed::Medium => PERIOD_RIPPLE_MED,
        Speed::Fast => PERIOD_RIPPLE_FAST,
    };
    let mut pkt = build_effect_header(
        EFFECT_FIREWORK_OR_RIPPLE,
        period,
        DIR_X_ZERO,
        DIR_Y_DEFAULT,
        DENSITY_DEFAULT,
        STEP_REACTIVE_DEFAULT,
        SUBMODE_REACTIVE_OR_STANDARD,
    );
    write_transient_palette(&mut pkt, palette);
    pkt
}

pub fn build_comet_packet(palette: TransientPalette, speed: Speed) -> [u8; REPORT_LEN] {
    let period = match speed {
        Speed::Slow => PERIOD_COMET_SLOW,
        Speed::Medium => PERIOD_COMET_MED,
        Speed::Fast => PERIOD_COMET_FAST,
    };
    let mut pkt = build_effect_header(
        EFFECT_LASER_OR_COMET,
        period,
        DIR_X_ZERO,
        DIR_Y_DEFAULT,
        DENSITY_HIGH,
        STEP_REACTIVE_DEFAULT,
        SUBMODE_REACTIVE_OR_STANDARD,
    );
    write_transient_palette(&mut pkt, palette);
    pkt
}

pub fn build_per_key_effect_packet(effect: PerKeyEffect) -> [u8; REPORT_LEN] {
    match effect {
        PerKeyEffect::Steady(color) => {
            let mut pkt = build_effect_header(
                EFFECT_STEADY,
                PERIOD_CUSTOMIZE_STEADY_FAST,
                DIR_X_CENTER,
                DIR_Y_DEFAULT,
                DENSITY_DEFAULT,
                STEP_DEFAULT_GLOBAL,
                SUBMODE_REACTIVE_OR_STANDARD,
            );
            write_template_1_steady(&mut pkt, color);
            pkt
        }
        PerKeyEffect::Breath { fg, bg, speed } => {
            let period = match speed {
                Speed::Slow => PERIOD_BREATH_CYCLE_SLOW,
                Speed::Medium => PERIOD_BREATH_CYCLE_MED,
                Speed::Fast => PERIOD_BREATH_CYCLE_FAST,
            };
            let mut pkt = build_effect_header(
                EFFECT_BREATH,
                period,
                DIR_X_ZERO,
                DIR_Y_DEFAULT,
                DENSITY_HIGH,
                STEP_REACTIVE_DEFAULT,
                SUBMODE_REACTIVE_OR_STANDARD,
            );
            write_template_2_breath(&mut pkt, fg, bg);
            pkt
        }
        PerKeyEffect::ColorCycle { palette, speed } => {
            let period = match speed {
                Speed::Slow => PERIOD_BREATH_CYCLE_SLOW,
                Speed::Medium => PERIOD_BREATH_CYCLE_MED,
                Speed::Fast => PERIOD_BREATH_CYCLE_FAST,
            };
            let mut pkt = build_effect_header(
                EFFECT_COLOR_CYCLE,
                period,
                DIR_X_CENTER,
                DIR_Y_DEFAULT,
                DENSITY_DEFAULT,
                STEP_CUSTOMIZE_ZERO,
                SUBMODE_REACTIVE_OR_STANDARD,
            );
            write_cycle_palette(&mut pkt, palette);
            pkt
        }
        PerKeyEffect::Wave { palette, speed } => {
            let period = match speed {
                Speed::Slow => PERIOD_WAVE_SLOW,
                Speed::Medium => PERIOD_WAVE_MED,
                Speed::Fast => PERIOD_WAVE_FAST,
            };
            let mut pkt = build_effect_header(
                EFFECT_WAVE,
                period,
                DIR_X_CENTER,
                DIR_Y_DEFAULT,
                DENSITY_DEFAULT,
                STEP_CUSTOMIZE_ZERO,
                SUBMODE_AUTONOMOUS,
            );
            write_cycle_palette(&mut pkt, palette);
            pkt
        }
        PerKeyEffect::Dance { speed } => {
            let period = match speed {
                Speed::Slow => PERIOD_DANCE_SLOW,
                Speed::Medium => PERIOD_DANCE_MED,
                Speed::Fast => PERIOD_DANCE_FAST,
            };
            build_effect_header(
                EFFECT_DANCE,
                period,
                DIR_X_CENTER,
                DIR_Y_DEFAULT,
                DENSITY_DEFAULT,
                STEP_DEFAULT_GLOBAL,
                SUBMODE_REACTIVE_OR_STANDARD,
            )
        }
        PerKeyEffect::Reactive { color, speed } => {
            let period = match speed {
                Speed::Slow => PERIOD_REACTIVE_SLOW,
                Speed::Medium => PERIOD_REACTIVE_MED,
                Speed::Fast => PERIOD_REACTIVE_FAST,
            };
            let mut pkt = build_effect_header(
                EFFECT_STARLIGHT_OR_REACTIVE,
                period,
                DIR_X_ZERO,
                DIR_Y_DEFAULT,
                DENSITY_HIGH,
                STEP_REACTIVE_DEFAULT,
                SUBMODE_REACTIVE_OR_STANDARD,
            );
            write_template_5a_single(&mut pkt, color);
            pkt
        }
        PerKeyEffect::Ripple { palette, speed } => {
            let period = match speed {
                Speed::Slow => PERIOD_RIPPLE_SLOW,
                Speed::Medium => PERIOD_RIPPLE_MED,
                Speed::Fast => PERIOD_RIPPLE_FAST,
            };
            let mut pkt = build_effect_header(
                EFFECT_FIREWORK_OR_RIPPLE,
                period,
                DIR_X_CENTER,
                DIR_Y_DEFAULT,
                DENSITY_DEFAULT,
                STEP_CUSTOMIZE_ZERO,
                SUBMODE_REACTIVE_OR_STANDARD,
            );
            write_transient_palette(&mut pkt, palette);
            pkt
        }
        PerKeyEffect::Disabled => build_effect_header(
            EFFECT_DANCE,
            PERIOD_DANCE_FAST,
            DIR_X_CENTER,
            DIR_Y_DEFAULT,
            DENSITY_DEFAULT,
            STEP_CUSTOMIZE_ZERO,
            SUBMODE_REACTIVE_OR_STANDARD,
        ),
    }
}

// ============================================================================
// Color Stop Encoding Helpers (Bytes 11..38)
// ============================================================================

fn write_template_1_steady(pkt: &mut [u8; REPORT_LEN], c: Rgb) {
    pkt[11..19].copy_from_slice(&[c.0, c.1, c.2, POS_100_PERCENT, c.0, c.1, c.2, POS_WRAP_0]);
}

fn write_template_2_breath(pkt: &mut [u8; REPORT_LEN], fg: Rgb, bg: Rgb) {
    pkt[11..23].copy_from_slice(&[
        fg.0,
        fg.1,
        fg.2,
        POS_50_PERCENT,
        bg.0,
        bg.1,
        bg.2,
        POS_100_PERCENT,
        fg.0,
        fg.1,
        fg.2,
        POS_WRAP_0,
    ]);
}

fn write_cycle_palette(pkt: &mut [u8; REPORT_LEN], palette: CyclePalette) {
    match palette {
        CyclePalette::Rainbow => {
            pkt[11..39].copy_from_slice(&RAINBOW_LOOP_STOPS);
        }
        CyclePalette::ThreeColor(c1, c2, c3) => {
            pkt[11..27].copy_from_slice(&[
                c1.0, c1.1, c1.2, POS_3COLOR_1,
                c2.0, c2.1, c2.2, POS_3COLOR_2,
                c3.0, c3.1, c3.2, POS_3COLOR_3,
                c1.0, c1.1, c1.2, POS_WRAP_0,
            ]);
        }
    }
}

fn write_template_5a_single(pkt: &mut [u8; REPORT_LEN], c: Rgb) {
    pkt[11..19].copy_from_slice(&[0x00, 0x00, 0x00, POS_50_PERCENT, c.0, c.1, c.2, POS_100_PERCENT]);
}

fn write_transient_palette(pkt: &mut [u8; REPORT_LEN], palette: TransientPalette) {
    match palette {
        TransientPalette::Rainbow => {
            pkt[11..39].copy_from_slice(&BLACK_LEAD_IN_RAINBOW_STOPS);
        }
        TransientPalette::SingleColor(c) => {
            write_template_5a_single(pkt, c);
        }
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{MockTransport, RecordedOperation};

    #[test]
    fn test_set_breath_cyan_fg_white_bg_slow_wire_format() {
        let mut driver = MsiKeyboardDriver::new(MockTransport::new());

        // User prompt example: mode Breath, color FG (0, 255, 255), color BG (255, 255, 255), speed slow
        driver
            .set_breath(Rgb(0, 255, 255), Rgb(255, 255, 255), Speed::Slow)
            .expect("set_breath should succeed");

        let ops = &driver.transport().operations;
        assert_eq!(ops.len(), 4, "Must execute 2 SET_REPORT + 2 GET_REPORT in lockstep");

        // 1. 02 01 Global Key Mask
        let mut expected_mask = [0u8; REPORT_LEN];
        expected_mask[0] = 0x02;
        expected_mask[1] = 0x01;
        expected_mask[2..34].fill(0xff);
        assert_eq!(
            ops[0],
            RecordedOperation::SetReport {
                w_value: WVALUE_REPORT_ID_2,
                payload: expected_mask
            }
        );
        assert_eq!(
            ops[1],
            RecordedOperation::GetReport {
                w_value: WVALUE_REPORT_ID_1,
                length: 64
            }
        );

        // 2. 02 02 Breath Slow (period = 1200 = 0x04b0 -> b0 04), FG(0,255,255)@0x32, BG(255,255,255)@0x64, FG@0x00
        let mut expected_eff = [0u8; REPORT_LEN];
        expected_eff[0..23].copy_from_slice(&[
            0x02, 0x02, 0x02, 0xb0, 0x04, 0x32, 0x32, 0x0a, 0x01, 0x00, 0x00, // Header
            0x00, 0xff, 0xff, 0x32, // FG Cyan @ 50%
            0xff, 0xff, 0xff, 0x64, // BG White @ 100%
            0x00, 0xff, 0xff, 0x00, // Wrap FG Cyan @ 0%
        ]);
        assert_eq!(
            ops[2],
            RecordedOperation::SetReport {
                w_value: WVALUE_REPORT_ID_2,
                payload: expected_eff
            }
        );
        assert_eq!(
            ops[3],
            RecordedOperation::GetReport {
                w_value: WVALUE_REPORT_ID_1,
                length: 64
            }
        );
    }

    #[test]
    fn test_autonomous_vs_reactive_shared_effect_ids() {
        let mut driver = MsiKeyboardDriver::new(MockTransport::new());

        // StarLight (ID 0x05, Sub 0x01) vs Reactive (ID 0x05, Sub 0x00)
        driver
            .set_starlight(TransientPalette::SingleColor(Rgb::ORANGE), Speed::Fast)
            .unwrap();
        driver
            .set_reactive(TransientPalette::SingleColor(Rgb::WHITE), Speed::Fast)
            .unwrap();

        let reports = driver.transport().set_reports();
        assert_eq!(reports.len(), 4);

        let starlight_pkt = reports[1].1;
        assert_eq!(
            &starlight_pkt[0..19],
            &[
                0x02, 0x02, 0x05, 0x2c, 0x01, 0x00, 0x32, 0x0a, 0x01, 0x01, 0x00,
                0x00, 0x00, 0x00, 0x32, 0xff, 0xb9, 0x00, 0x64
            ]
        );

        let reactive_pkt = reports[3].1;
        assert_eq!(
            &reactive_pkt[0..19],
            &[
                0x02, 0x02, 0x05, 0x64, 0x00, 0x00, 0x32, 0x50, 0x02, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x32, 0xff, 0xff, 0xff, 0x64
            ]
        );
    }

    #[test]
    fn test_dance_mode_sends_mandatory_pre_clear() {
        let mut driver = MsiKeyboardDriver::new(MockTransport::new());
        driver.set_dance(Speed::Medium).unwrap();

        let reports = driver.transport().set_reports();
        assert_eq!(reports.len(), 4, "Dance mode requires 2 pairs: Clear + Dance");

        // First 02 02 is all-zero Off clear (02 02 06 00...)
        let clear_eff = reports[1].1;
        assert_eq!(&clear_eff[0..3], &[0x02, 0x02, 0x06]);
        assert!(clear_eff[3..].iter().all(|&b| b == 0));

        // Second 02 02 is Dance Medium (02 02 00 c8 00 00 32 0a 01 00 00)
        let dance_eff = reports[3].1;
        assert_eq!(
            &dance_eff[0..11],
            &[0x02, 0x02, 0x00, 0xc8, 0x00, 0x00, 0x32, 0x0a, 0x01, 0x00, 0x00]
        );
    }

    #[test]
    fn test_music_mode_uses_65_byte_get_report_on_0x0302() {
        let mut driver = MsiKeyboardDriver::new(MockTransport::new());
        driver.start_music_mode().unwrap();
        driver.send_music_frame(&[0u8; 62]).unwrap();

        let ops = &driver.transport().operations;
        // Verify 02 05 init packet and 65-byte GET_REPORT(0x0302)
        let has_music_init_readback = ops.windows(2).any(|w| {
            matches!(
                (&w[0], &w[1]),
                (
                    RecordedOperation::SetReport {
                        w_value: WVALUE_REPORT_ID_2,
                        payload
                    },
                    RecordedOperation::GetReport {
                        w_value: WVALUE_REPORT_ID_2,
                        length: 65
                    }
                ) if payload[0] == 0x02 && payload[1] == 0x05 && payload[20] == 0xff
            )
        });
        assert!(has_music_init_readback, "02 05 must be followed by 65B GET_REPORT(0x0302)");

        // Verify 02 06 audio frame packet and 65-byte GET_REPORT(0x0302)
        let has_music_frame_readback = ops.windows(2).any(|w| {
            matches!(
                (&w[0], &w[1]),
                (
                    RecordedOperation::SetReport {
                        w_value: WVALUE_REPORT_ID_2,
                        payload
                    },
                    RecordedOperation::GetReport {
                        w_value: WVALUE_REPORT_ID_2,
                        length: 65
                    }
                ) if payload[0] == 0x02 && payload[1] == 0x06
            )
        });
        assert!(has_music_frame_readback, "02 06 must be followed by 65B GET_REPORT(0x0302)");
    }

    #[test]
    fn test_customize_98_keys_esc_steady_white_and_union_mask() {
        let mut driver = MsiKeyboardDriver::new(MockTransport::new());

        driver
            .set_full_98_key_customize(false, |coord| {
                if coord == KEY_ESC_COORD {
                    PerKeyEffect::Steady(Rgb::WHITE)
                } else {
                    PerKeyEffect::Dance { speed: Speed::Fast }
                }
            })
            .unwrap();

        let reports = driver.transport().set_reports();
        assert_eq!(reports.len(), 98 * 2, "98 keys without initial clear = 196 SET_REPORTs");

        // Key 0 in serialization order is Esc (Packet Byte 7 = 0x02)
        let first_mask = reports[0].1;
        assert_eq!(first_mask[0], 0x02);
        assert_eq!(first_mask[1], 0x01);
        assert_eq!(first_mask[7], 0x02);

        // First effect is Customize Steady White (02 02 01 64 00 32 32 0a 01 00 00 ff ff ff 64 ff ff ff 00)
        let first_eff = reports[1].1;
        assert_eq!(
            &first_eff[0..19],
            &[
                0x02, 0x02, 0x01, 0x64, 0x00, 0x32, 0x32, 0x0a, 0x01, 0x00, 0x00,
                0xff, 0xff, 0xff, 0x64, 0xff, 0xff, 0xff, 0x00
            ]
        );

        // Verify bitwise OR union of all 98 transmitted key masks equals VALID_98_KEY_MASK
        let mut union_mask = [0u8; 32];
        for idx in (0..reports.len()).step_by(2) {
            let mask_pkt = reports[idx].1;
            for b in 0..32 {
                union_mask[b] |= mask_pkt[2 + b];
            }
        }
        assert_eq!(union_mask, VALID_98_KEY_MASK);
    }

    #[test]
    fn test_flash_apply_sends_02a0_twice_with_mode_specific_pause() {
        let mut driver = MsiKeyboardDriver::new(MockTransport::new());
        driver.apply_to_flash(false).unwrap();

        let expected_apply = build_flash_apply_packet();
        assert_eq!(
            driver.transport().operations,
            vec![
                RecordedOperation::SetReport {
                    w_value: WVALUE_REPORT_ID_2,
                    payload: expected_apply
                },
                RecordedOperation::GetReport {
                    w_value: WVALUE_REPORT_ID_1,
                    length: 64
                },
                RecordedOperation::Sleep(DELAY_FLASH_APPLY_GLOBAL),
                RecordedOperation::SetReport {
                    w_value: WVALUE_REPORT_ID_2,
                    payload: expected_apply
                },
                RecordedOperation::GetReport {
                    w_value: WVALUE_REPORT_ID_1,
                    length: 64
                },
            ]
        );
    }

    #[test]
    fn test_per_key_ripple_and_reactive_packet_headers() {
        // Per-Key Ripple matches wire Frames 1121 & 1129:
        // 02 02 07 2c 01 32 32 0a 00 00 00 00 00 00 32 ff 00 00 64...
        let ripple_pkt = build_per_key_effect_packet(PerKeyEffect::Ripple {
            palette: TransientPalette::SingleColor(Rgb::RED),
            speed: Speed::Medium,
        });
        assert_eq!(
            &ripple_pkt[0..19],
            &[
                0x02, 0x02, 0x07, 0x2c, 0x01, 0x32, 0x32, 0x0a, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x32, 0xff, 0x00, 0x00, 0x64
            ]
        );

        // Per-Key Reactive matches wire Frames 969–993:
        // 02 02 05 2c 01 00 32 50 02 00 00 00 00 00 32 ff ff ff 64...
        let reactive_pkt = build_per_key_effect_packet(PerKeyEffect::Reactive {
            color: Rgb::WHITE,
            speed: Speed::Medium,
        });
        assert_eq!(
            &reactive_pkt[0..19],
            &[
                0x02, 0x02, 0x05, 0x2c, 0x01, 0x00, 0x32, 0x50, 0x02, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x32, 0xff, 0xff, 0xff, 0x64
            ]
        );
    }
}
