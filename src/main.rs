mod cli;

use std::process::ExitCode;

use clap::Parser;
use cli::{run_command, Cli};
use rgb_controller_msi_creator_a16::driver::MsiKeyboardDriver;
use rgb_controller_msi_creator_a16::transport::{MockTransport, NusbTransport};

fn main() -> ExitCode {
    let cli = Cli::parse();

    if cli.dry_run {
        let mut driver = MsiKeyboardDriver::new(MockTransport::new());
        if let Err(e) = run_command(&mut driver, &cli.command) {
            eprintln!("Driver error during dry-run: {e}");
            return ExitCode::FAILURE;
        }
        println!(
            "Dry-run completed for command '{}': recorded {} USB control operations ({} SET_REPORTs).",
            cli.command,
            driver.transport().operations.len(),
            driver.transport().set_reports().len()
        );
        for (idx, (w_value, payload)) in driver.transport().set_reports().iter().enumerate() {
            println!(
                "  SET_REPORT #{idx} (wValue=0x{w_value:04x}): {:02x?}",
                &payload[..24]
            );
        }
        return ExitCode::SUCCESS;
    }

    match NusbTransport::open() {
        Ok(transport) => {
            let mut driver = MsiKeyboardDriver::new(transport);
            if let Err(e) = run_command(&mut driver, &cli.command) {
                eprintln!(
                    "USB communication error while executing '{}': {e}",
                    cli.command
                );
                return ExitCode::FAILURE;
            }
            println!(
                "Successfully executed command '{}' on MSI RGB Keyboard (0x1462:0x1565).",
                cli.command
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Failed to open USB device: {e}");
            eprintln!("Tip: Pass `--dry-run` to test command payload generation without hardware.");
            ExitCode::FAILURE
        }
    }
}
