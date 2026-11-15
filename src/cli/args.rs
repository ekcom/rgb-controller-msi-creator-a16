use std::str::FromStr;

use clap::{Parser, ValueEnum};
use rgb_controller_msi_creator_a16::driver::{Rgb, Speed, WaveDirection};

use super::commands::Commands;

/// Top-level CLI argument configuration and root parser.
#[derive(Parser, Debug)]
#[command(
    name = "rgb-controller-msi-creator-a16",
    about = "Userspace USB HID driver & CLI for MSI Per-Key RGB Keyboard Controller (1462:1565)",
    version
)]
pub struct Cli {
    /// Dry run: execute the command on a mock transport and print captured USB control transfers without touching hardware
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Command to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// CLI color wrapper supporting hex (`#RRGGBB`, `RRGGBB`), comma-separated (`R,G,B`), and common names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CliColor(pub Rgb);

impl FromStr for CliColor {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_lowercase();
        let rgb = match normalized.as_str() {
            "white" => Rgb::WHITE,
            "red" => Rgb::RED,
            "green" => Rgb::GREEN,
            "blue" => Rgb::BLUE,
            "cyan" => Rgb::CYAN,
            "light-cyan" | "lightcyan" => Rgb::LIGHT_CYAN,
            "yellow" => Rgb::YELLOW,
            "magenta" | "pink" => Rgb::MAGENTA,
            "orange" => Rgb::ORANGE,
            "warm-orange" | "warmorange" => Rgb::WARM_ORANGE,
            "black" | "off" => Rgb(0, 0, 0),
            hex_or_dec => {
                let stripped = hex_or_dec
                    .strip_prefix('#')
                    .or_else(|| hex_or_dec.strip_prefix("0x"))
                    .unwrap_or(hex_or_dec);

                if stripped.len() == 6 && stripped.chars().all(|c| c.is_ascii_hexdigit()) {
                    let r = u8::from_str_radix(&stripped[0..2], 16)
                        .map_err(|e| format!("Invalid red hex in '{s}': {e}"))?;
                    let g = u8::from_str_radix(&stripped[2..4], 16)
                        .map_err(|e| format!("Invalid green hex in '{s}': {e}"))?;
                    let b = u8::from_str_radix(&stripped[4..6], 16)
                        .map_err(|e| format!("Invalid blue hex in '{s}': {e}"))?;
                    Rgb(r, g, b)
                } else if stripped.contains(',') {
                    let parts: Vec<&str> = stripped.split(',').map(str::trim).collect();
                    if parts.len() == 3 {
                        let r = parts[0]
                            .parse::<u8>()
                            .map_err(|e| format!("Invalid red component in '{s}': {e}"))?;
                        let g = parts[1]
                            .parse::<u8>()
                            .map_err(|e| format!("Invalid green component in '{s}': {e}"))?;
                        let b = parts[2]
                            .parse::<u8>()
                            .map_err(|e| format!("Invalid blue component in '{s}': {e}"))?;
                        Rgb(r, g, b)
                    } else {
                        return Err(format!(
                            "Expected 3 comma-separated components 'R,G,B', got '{s}'"
                        ));
                    }
                } else {
                    return Err(format!(
                        "Unrecognized color '{s}'. Use '#RRGGBB', 'R,G,B', or a name like 'red', 'cyan', 'white'."
                    ));
                }
            }
        };
        Ok(CliColor(rgb))
    }
}

impl std::fmt::Display for CliColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0 .0, self.0 .1, self.0 .2)
    }
}

/// CLI animation speed wrapper.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliSpeed {
    Slow,
    #[value(alias = "med")]
    Medium,
    Fast,
}

impl From<CliSpeed> for Speed {
    fn from(s: CliSpeed) -> Self {
        match s {
            CliSpeed::Slow => Speed::Slow,
            CliSpeed::Medium => Speed::Medium,
            CliSpeed::Fast => Speed::Fast,
        }
    }
}

impl std::fmt::Display for CliSpeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliSpeed::Slow => write!(f, "slow"),
            CliSpeed::Medium => write!(f, "medium"),
            CliSpeed::Fast => write!(f, "fast"),
        }
    }
}

/// Wave propagation direction.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliWaveDirection {
    #[value(name = "left-to-right", alias = "ltr")]
    LeftToRight,
    #[value(name = "right-to-left", alias = "rtl")]
    RightToLeft,
}

impl From<CliWaveDirection> for WaveDirection {
    fn from(d: CliWaveDirection) -> Self {
        match d {
            CliWaveDirection::LeftToRight => WaveDirection::LeftToRight,
            CliWaveDirection::RightToLeft => WaveDirection::RightToLeft,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_color_parsing_valid() {
        assert_eq!(CliColor::from_str("red").unwrap().0, Rgb::RED);
        assert_eq!(CliColor::from_str("WHITE").unwrap().0, Rgb::WHITE);
        assert_eq!(CliColor::from_str("cyan").unwrap().0, Rgb::CYAN);
        assert_eq!(CliColor::from_str("#ff0000").unwrap().0, Rgb::RED);
        assert_eq!(CliColor::from_str("0x00ff00").unwrap().0, Rgb::GREEN);
        assert_eq!(CliColor::from_str("0000ff").unwrap().0, Rgb::BLUE);
        assert_eq!(CliColor::from_str("255, 185, 0").unwrap().0, Rgb::ORANGE);
    }

    #[test]
    fn test_cli_color_parsing_errors() {
        assert!(CliColor::from_str("not_a_color").is_err());
        assert!(CliColor::from_str("#12345").is_err());
        assert!(CliColor::from_str("255, 255").is_err());
        assert!(CliColor::from_str("256, 0, 0").is_err());
    }

    #[test]
    fn test_cli_requires_subcommand() {
        assert!(Cli::try_parse_from(["app"]).is_err());
        assert!(Cli::try_parse_from(["app", "--dry-run"]).is_err());
    }

    #[test]
    fn test_cli_speed_and_direction_conversions() {
        assert_eq!(Speed::from(CliSpeed::Slow), Speed::Slow);
        assert_eq!(Speed::from(CliSpeed::Medium), Speed::Medium);
        assert_eq!(Speed::from(CliSpeed::Fast), Speed::Fast);

        assert_eq!(
            WaveDirection::from(CliWaveDirection::LeftToRight),
            WaveDirection::LeftToRight
        );
        assert_eq!(
            WaveDirection::from(CliWaveDirection::RightToLeft),
            WaveDirection::RightToLeft
        );
    }
}
