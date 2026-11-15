use clap::{Subcommand, ValueEnum};
use rgb_controller_msi_creator_a16::driver::{
    build_off_standard_packet, CyclePalette, MsiKeyboardDriver, PerKeyEffect, Rgb, Speed,
    TransientPalette, WaveDirection,
};
use rgb_controller_msi_creator_a16::protocol::KEY_ESC_COORD;
use rgb_controller_msi_creator_a16::transport::{HidControlTransport, TransportError};

use super::args::{CliColor, CliSpeed, CliWaveDirection};

/// Per-key Customize layout presets.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomizePreset {
    /// Esc key steady white, remaining 97 keys set to dance (fast)
    #[value(name = "esc-white-rest-dance")]
    EscWhiteRestDance,
    /// All 98 keys set to dance (fast)
    #[value(name = "all-dance")]
    AllDance,
    /// All 98 keys set to steady red
    #[value(name = "all-steady-red")]
    AllSteadyRed,
}

/// Pre-configured reference capture demo presets.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetName {
    /// Breathing effect: cyan foreground, white background, slow speed
    #[value(name = "breath-cyan-white-slow")]
    BreathCyanWhiteSlow,
    /// Solid static red color across all keys
    #[value(name = "steady-red")]
    SteadyRed,
    /// Left-to-right animated wave with rainbow palette at medium speed
    #[value(name = "wave-rainbow-ltr")]
    WaveRainbowLtr,
    /// Random twinkling keys with rainbow palette at fast speed
    #[value(name = "starlight-rainbow-fast")]
    StarlightRainbowFast,
    /// Keypress illumination in white at fast speed
    #[value(name = "reactive-white-fast")]
    ReactiveWhiteFast,
    /// Built-in fast dance animation pattern
    #[value(name = "dance-fast")]
    DanceFast,
    /// Per-key layout: Esc key steady white, all other 97 keys set to fast dance
    #[value(name = "customize-esc-white-rest-dance")]
    CustomizeEscWhiteRestDance,
}

/// Subcommands supported by the keyboard controller.
#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Initialize keyboard controller session with standard startup handshake
    Init,

    /// Turn off all keyboard RGB lighting
    Off,

    /// Set solid static color on all keys
    Steady {
        /// Color as hex (e.g. '#ff0000', 'ff0000') or name ('red', 'cyan', 'white')
        #[arg(default_value = "red")]
        color: CliColor,
    },

    /// Set two-color pulsing breathing effect
    Breath {
        /// Foreground color
        #[arg(short = 'f', long, default_value = "cyan")]
        fg: CliColor,

        /// Background color
        #[arg(short = 'b', long, default_value = "white")]
        bg: CliColor,

        /// Animation speed preset
        #[arg(short, long, value_enum, default_value_t = CliSpeed::Slow)]
        speed: CliSpeed,
    },

    /// Set cycling color loop
    #[command(name = "color-cycle")]
    ColorCycle {
        /// Animation speed preset
        #[arg(short, long, value_enum, default_value_t = CliSpeed::Medium)]
        speed: CliSpeed,

        /// Custom 3 colors (if omitted, rainbow palette is used)
        #[arg(long, num_args = 3, value_names = ["COLOR1", "COLOR2", "COLOR3"])]
        colors: Option<Vec<CliColor>>,
    },

    /// Set animated color wave
    Wave {
        /// Wave animation speed
        #[arg(short, long, value_enum, default_value_t = CliSpeed::Medium)]
        speed: CliSpeed,

        /// Direction of wave travel
        #[arg(short, long, value_enum, default_value_t = CliWaveDirection::LeftToRight)]
        direction: CliWaveDirection,

        /// Custom 3 colors (if omitted, rainbow palette is used)
        #[arg(long, num_args = 3, value_names = ["COLOR1", "COLOR2", "COLOR3"])]
        colors: Option<Vec<CliColor>>,
    },

    /// Set built-in dance animation pattern
    Dance {
        /// Dance animation speed
        #[arg(short, long, value_enum, default_value_t = CliSpeed::Fast)]
        speed: CliSpeed,
    },

    /// Set random twinkling key effect
    Starlight {
        /// Animation speed preset
        #[arg(short, long, value_enum, default_value_t = CliSpeed::Fast)]
        speed: CliSpeed,

        /// Single color (if omitted, rainbow palette is used)
        #[arg(short, long)]
        color: Option<CliColor>,
    },

    /// Set expanding firework burst effect
    Firework {
        /// Animation speed preset
        #[arg(short, long, value_enum, default_value_t = CliSpeed::Fast)]
        speed: CliSpeed,

        /// Single color (if omitted, rainbow palette is used)
        #[arg(short, long)]
        color: Option<CliColor>,
    },

    /// Set beaming horizontal laser lines
    Laser {
        /// Animation speed preset
        #[arg(short, long, value_enum, default_value_t = CliSpeed::Fast)]
        speed: CliSpeed,

        /// Single color (if omitted, rainbow palette is used)
        #[arg(short, long)]
        color: Option<CliColor>,
    },

    /// Set key-press reactive illumination
    Reactive {
        /// Animation speed preset
        #[arg(short, long, value_enum, default_value_t = CliSpeed::Fast)]
        speed: CliSpeed,

        /// Single color (if omitted, rainbow palette is used)
        #[arg(short, long)]
        color: Option<CliColor>,
    },

    /// Set expanding circular wave on keypress
    Ripple {
        /// Animation speed preset
        #[arg(short, long, value_enum, default_value_t = CliSpeed::Fast)]
        speed: CliSpeed,

        /// Single color (if omitted, rainbow palette is used)
        #[arg(short, long)]
        color: Option<CliColor>,
    },

    /// Set shooting comet effect on keypress
    Comet {
        /// Animation speed preset
        #[arg(short, long, value_enum, default_value_t = CliSpeed::Fast)]
        speed: CliSpeed,

        /// Single color (if omitted, rainbow palette is used)
        #[arg(short, long)]
        color: Option<CliColor>,
    },

    /// Configure per-key Customize mode
    Customize {
        /// Preset layout to apply
        #[arg(short, long, value_enum, default_value_t = CustomizePreset::EscWhiteRestDance)]
        preset: CustomizePreset,

        /// Skip the initial pre-effect global clear (for incremental updates)
        #[arg(long)]
        no_clear: bool,
    },

    /// Persist active lighting configuration to non-volatile flash memory
    Apply {
        /// Pass if saving a 98-key per-key Customize layout (~640ms pause vs 85-90ms)
        #[arg(long)]
        customize: bool,
    },

    /// Discard unsaved changes and reset keyboard lighting
    #[command(alias = "discard-close")]
    Discard,

    /// Execute one of the preconfigured reference/capture demo presets
    Preset {
        /// Name of the preset to run
        #[arg(value_enum, default_value_t = PresetName::BreathCyanWhiteSlow)]
        name: PresetName,
    },
}

impl std::fmt::Display for Commands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Commands::Init => write!(f, "init"),
            Commands::Off => write!(f, "off"),
            Commands::Steady { color } => write!(f, "steady (color={color})"),
            Commands::Breath { fg, bg, speed } => {
                write!(f, "breath (fg={fg}, bg={bg}, speed={speed})")
            }
            Commands::ColorCycle { speed, colors } => {
                if let Some(c) = colors.as_deref().filter(|c| c.len() == 3) {
                    write!(
                        f,
                        "color-cycle (colors=[{}, {}, {}], speed={speed})",
                        c[0], c[1], c[2]
                    )
                } else {
                    write!(f, "color-cycle (palette=rainbow, speed={speed})")
                }
            }
            Commands::Wave {
                speed,
                direction,
                colors,
            } => {
                let dir_str = match direction {
                    CliWaveDirection::LeftToRight => "left-to-right",
                    CliWaveDirection::RightToLeft => "right-to-left",
                };
                if let Some(c) = colors.as_deref().filter(|c| c.len() == 3) {
                    write!(
                        f,
                        "wave (colors=[{}, {}, {}], speed={speed}, dir={dir_str})",
                        c[0], c[1], c[2]
                    )
                } else {
                    write!(f, "wave (palette=rainbow, speed={speed}, dir={dir_str})")
                }
            }
            Commands::Dance { speed } => write!(f, "dance (speed={speed})"),
            Commands::Starlight { speed, color } => {
                if let Some(c) = color {
                    write!(f, "starlight (color={c}, speed={speed})")
                } else {
                    write!(f, "starlight (palette=rainbow, speed={speed})")
                }
            }
            Commands::Firework { speed, color } => {
                if let Some(c) = color {
                    write!(f, "firework (color={c}, speed={speed})")
                } else {
                    write!(f, "firework (palette=rainbow, speed={speed})")
                }
            }
            Commands::Laser { speed, color } => {
                if let Some(c) = color {
                    write!(f, "laser (color={c}, speed={speed})")
                } else {
                    write!(f, "laser (palette=rainbow, speed={speed})")
                }
            }
            Commands::Reactive { speed, color } => {
                if let Some(c) = color {
                    write!(f, "reactive (color={c}, speed={speed})")
                } else {
                    write!(f, "reactive (palette=rainbow, speed={speed})")
                }
            }
            Commands::Ripple { speed, color } => {
                if let Some(c) = color {
                    write!(f, "ripple (color={c}, speed={speed})")
                } else {
                    write!(f, "ripple (palette=rainbow, speed={speed})")
                }
            }
            Commands::Comet { speed, color } => {
                if let Some(c) = color {
                    write!(f, "comet (color={c}, speed={speed})")
                } else {
                    write!(f, "comet (palette=rainbow, speed={speed})")
                }
            }
            Commands::Customize { preset, no_clear } => {
                write!(f, "customize (preset={preset:?}, no_clear={no_clear})")
            }
            Commands::Apply { customize } => write!(f, "apply (customize={customize})"),
            Commands::Discard => write!(f, "discard"),
            Commands::Preset { name } => write!(f, "preset ({name:?})"),
        }
    }
}

/// Executes a pre-configured reference preset on the driver.
pub fn execute_preset<T: HidControlTransport>(
    driver: &mut MsiKeyboardDriver<T>,
    name: PresetName,
) -> Result<(), TransportError> {
    match name {
        PresetName::BreathCyanWhiteSlow => {
            driver.set_breath(Rgb::CYAN, Rgb::WHITE, Speed::Slow)?;
        }
        PresetName::SteadyRed => {
            driver.set_steady(Rgb::RED)?;
        }
        PresetName::WaveRainbowLtr => {
            driver.set_wave(
                CyclePalette::Rainbow,
                Speed::Medium,
                WaveDirection::LeftToRight,
            )?;
        }
        PresetName::StarlightRainbowFast => {
            driver.set_starlight(TransientPalette::Rainbow, Speed::Fast)?;
        }
        PresetName::ReactiveWhiteFast => {
            driver.set_reactive(TransientPalette::SingleColor(Rgb::WHITE), Speed::Fast)?;
        }
        PresetName::DanceFast => {
            driver.set_dance(Speed::Fast)?;
        }
        PresetName::CustomizeEscWhiteRestDance => {
            driver.set_full_98_key_customize(true, |coord| {
                if coord == KEY_ESC_COORD {
                    PerKeyEffect::Steady(Rgb::WHITE)
                } else {
                    PerKeyEffect::Dance { speed: Speed::Fast }
                }
            })?;
        }
    }
    Ok(())
}

/// Dispatches a parsed CLI command to the corresponding [`MsiKeyboardDriver`] method.
pub fn run_command<T: HidControlTransport>(
    driver: &mut MsiKeyboardDriver<T>,
    command: &Commands,
) -> Result<(), TransportError> {
    match command {
        Commands::Init => {
            let initial_off = build_off_standard_packet();
            driver.initialize_session(true, &initial_off)?;
        }
        Commands::Off => {
            driver.set_off()?;
        }
        Commands::Steady { color } => {
            driver.set_steady(color.0)?;
        }
        Commands::Breath { fg, bg, speed } => {
            driver.set_breath(fg.0, bg.0, (*speed).into())?;
        }
        Commands::ColorCycle { speed, colors } => {
            let palette = match colors {
                Some(c) if c.len() == 3 => CyclePalette::ThreeColor(c[0].0, c[1].0, c[2].0),
                _ => CyclePalette::Rainbow,
            };
            driver.set_color_cycle(palette, (*speed).into())?;
        }
        Commands::Wave {
            speed,
            direction,
            colors,
        } => {
            let palette = match colors {
                Some(c) if c.len() == 3 => CyclePalette::ThreeColor(c[0].0, c[1].0, c[2].0),
                _ => CyclePalette::Rainbow,
            };
            driver.set_wave(palette, (*speed).into(), (*direction).into())?;
        }
        Commands::Dance { speed } => {
            driver.set_dance((*speed).into())?;
        }
        Commands::Starlight { speed, color } => {
            let palette = match color {
                Some(c) => TransientPalette::SingleColor(c.0),
                None => TransientPalette::Rainbow,
            };
            driver.set_starlight(palette, (*speed).into())?;
        }
        Commands::Firework { speed, color } => {
            let palette = match color {
                Some(c) => TransientPalette::SingleColor(c.0),
                None => TransientPalette::Rainbow,
            };
            driver.set_firework(palette, (*speed).into())?;
        }
        Commands::Laser { speed, color } => {
            let palette = match color {
                Some(c) => TransientPalette::SingleColor(c.0),
                None => TransientPalette::Rainbow,
            };
            driver.set_laser(palette, (*speed).into())?;
        }
        Commands::Reactive { speed, color } => {
            let palette = match color {
                Some(c) => TransientPalette::SingleColor(c.0),
                None => TransientPalette::Rainbow,
            };
            driver.set_reactive(palette, (*speed).into())?;
        }
        Commands::Ripple { speed, color } => {
            let palette = match color {
                Some(c) => TransientPalette::SingleColor(c.0),
                None => TransientPalette::Rainbow,
            };
            driver.set_ripple(palette, (*speed).into())?;
        }
        Commands::Comet { speed, color } => {
            let palette = match color {
                Some(c) => TransientPalette::SingleColor(c.0),
                None => TransientPalette::Rainbow,
            };
            driver.set_comet(palette, (*speed).into())?;
        }
        Commands::Customize { preset, no_clear } => match preset {
            CustomizePreset::EscWhiteRestDance => {
                driver.set_full_98_key_customize(!no_clear, |coord| {
                    if coord == KEY_ESC_COORD {
                        PerKeyEffect::Steady(Rgb::WHITE)
                    } else {
                        PerKeyEffect::Dance { speed: Speed::Fast }
                    }
                })?;
            }
            CustomizePreset::AllDance => {
                driver.set_full_98_key_customize(!no_clear, |_| PerKeyEffect::Dance {
                    speed: Speed::Fast,
                })?;
            }
            CustomizePreset::AllSteadyRed => {
                driver.set_full_98_key_customize(!no_clear, |_| PerKeyEffect::Steady(Rgb::RED))?;
            }
        },
        Commands::Apply { customize } => {
            driver.apply_to_flash(*customize)?;
        }
        Commands::Discard => {
            driver.discard_and_close()?;
        }
        Commands::Preset { name } => {
            execute_preset(driver, *name)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use rgb_controller_msi_creator_a16::transport::MockTransport;

    use super::*;
    use crate::cli::args::Cli;

    #[test]
    fn test_subcommand_parsing() {
        let cli = Cli::parse_from(["app", "steady", "blue"]);
        match cli.command {
            Commands::Steady { color } => assert_eq!(color.0, Rgb::BLUE),
            other => panic!("Expected Steady, got {other:?}"),
        }

        let cli = Cli::parse_from([
            "app", "breath", "--fg", "red", "--bg", "green", "--speed", "fast",
        ]);
        match cli.command {
            Commands::Breath { fg, bg, speed } => {
                assert_eq!(fg.0, Rgb::RED);
                assert_eq!(bg.0, Rgb::GREEN);
                assert_eq!(speed, CliSpeed::Fast);
            }
            other => panic!("Expected Breath, got {other:?}"),
        }

        let cli = Cli::parse_from(["app", "wave", "--direction", "rtl", "--speed", "medium"]);
        match cli.command {
            Commands::Wave {
                speed, direction, ..
            } => {
                assert_eq!(speed, CliSpeed::Medium);
                assert_eq!(direction, CliWaveDirection::RightToLeft);
            }
            other => panic!("Expected Wave, got {other:?}"),
        }

        let cli = Cli::parse_from(["app", "preset", "dance-fast"]);
        match cli.command {
            Commands::Preset { name } => assert_eq!(name, PresetName::DanceFast),
            other => panic!("Expected Preset, got {other:?}"),
        }

        let cli = Cli::parse_from(["app", "preset"]);
        match cli.command {
            Commands::Preset { name } => assert_eq!(name, PresetName::BreathCyanWhiteSlow),
            other => panic!("Expected default BreathCyanWhiteSlow, got {other:?}"),
        }

        let cli = Cli::parse_from(["app", "apply", "--customize"]);
        match cli.command {
            Commands::Apply { customize } => assert!(customize),
            other => panic!("Expected Apply, got {other:?}"),
        }

        let cli = Cli::parse_from(["app", "discard"]);
        match cli.command {
            Commands::Discard => {}
            other => panic!("Expected Discard, got {other:?}"),
        }

        let cli = Cli::parse_from(["app", "reactive", "--color", "red"]);
        match cli.command {
            Commands::Reactive { speed, color } => {
                assert_eq!(speed, CliSpeed::Fast);
                assert_eq!(color, Some(CliColor(Rgb::RED)));
            }
            other => panic!("Expected Reactive, got {other:?}"),
        }

        let cli = Cli::parse_from(["app", "reactive"]);
        match cli.command {
            Commands::Reactive { speed, color } => {
                assert_eq!(speed, CliSpeed::Fast);
                assert_eq!(color, None);
            }
            other => panic!("Expected Reactive, got {other:?}"),
        }

        let cli = Cli::parse_from(["app", "customize", "--no-clear"]);
        match cli.command {
            Commands::Customize { preset, no_clear } => {
                assert_eq!(preset, CustomizePreset::EscWhiteRestDance);
                assert!(no_clear);
            }
            other => panic!("Expected Customize, got {other:?}"),
        }
    }

    #[test]
    fn test_commands_display_safety() {
        let cmd = Commands::ColorCycle {
            speed: CliSpeed::Medium,
            colors: Some(vec![CliColor(Rgb::RED)]), // Fewer than 3 items
        };
        assert_eq!(
            cmd.to_string(),
            "color-cycle (palette=rainbow, speed=medium)"
        );

        let cmd = Commands::Wave {
            speed: CliSpeed::Fast,
            direction: CliWaveDirection::LeftToRight,
            colors: Some(vec![CliColor(Rgb::RED), CliColor(Rgb::GREEN)]), // Fewer than 3 items
        };
        assert_eq!(
            cmd.to_string(),
            "wave (palette=rainbow, speed=fast, dir=left-to-right)"
        );

        let cmd = Commands::Reactive {
            speed: CliSpeed::Fast,
            color: None,
        };
        assert_eq!(cmd.to_string(), "reactive (palette=rainbow, speed=fast)");

        let cmd = Commands::Customize {
            preset: CustomizePreset::EscWhiteRestDance,
            no_clear: true,
        };
        assert_eq!(
            cmd.to_string(),
            "customize (preset=EscWhiteRestDance, no_clear=true)"
        );
    }

    #[test]
    fn test_run_command_steady_execution() {
        let mut driver = MsiKeyboardDriver::new(MockTransport::new());
        let cmd = Commands::Steady {
            color: CliColor(Rgb::RED),
        };
        assert!(run_command(&mut driver, &cmd).is_ok());
        assert_eq!(driver.transport().set_reports().len(), 2);
    }

    #[test]
    fn test_run_command_preset_execution() {
        let mut driver = MsiKeyboardDriver::new(MockTransport::new());
        let cmd = Commands::Preset {
            name: PresetName::DanceFast,
        };
        assert!(run_command(&mut driver, &cmd).is_ok());
        // Dance sends pre-clear (2 reports) + dance (2 reports) = 4 SET_REPORTs
        assert_eq!(driver.transport().set_reports().len(), 4);
    }
}
