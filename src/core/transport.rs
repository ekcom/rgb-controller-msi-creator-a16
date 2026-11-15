use std::collections::VecDeque;
use std::thread;
use std::time::Duration;

use nusb::transfer::{ControlIn, ControlOut, ControlType, Recipient, TransferError};
use nusb::MaybeFuture;
use thiserror::Error;

use super::protocol::{
    HID_REQ_GET_REPORT, HID_REQ_SET_IDLE, HID_REQ_SET_REPORT, INTERFACE_NUMBER, PRODUCT_ID,
    REPORT_LEN, USB_CONTROL_TIMEOUT, VENDOR_ID, WINDEX_INTERFACE_0, WVALUE_SET_IDLE_ALL,
};

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Target MSI RGB Keyboard Controller (0x{0:04x}:0x{1:04x}) not found on USB bus")]
    DeviceNotFound(u16, u16),

    #[error("USB enumeration or interface error: {0}")]
    UsbError(#[from] nusb::Error),

    #[error("USB control transfer failed: {0}")]
    TransferError(#[from] TransferError),

    #[error("Unexpected GET_REPORT readback length: expected {expected} bytes, got {actual}")]
    ReadbackLengthMismatch { expected: usize, actual: usize },

    #[error("Mock transport error: {0}")]
    Mock(String),
}

/// Abstract USB HID Control Endpoint 0 transport interface.
pub trait HidControlTransport {
    /// Issues a USB HID Class-Specific `SET_IDLE` (bRequest = 0x0a, wValue = 0x0000, wLength = 0).
    fn set_idle(&mut self) -> Result<(), TransportError>;

    /// Issues a 64-byte USB HID `SET_REPORT` (bRequest = 0x09) to `wValue` (0x0301 or 0x0302).
    fn set_report(
        &mut self,
        w_value: u16,
        payload: &[u8; REPORT_LEN],
    ) -> Result<(), TransportError>;

    /// Issues a USB HID `GET_REPORT` (bRequest = 0x01) from `wValue` (0x0301 or 0x0302)
    /// with the specified `length` (64 bytes for standard commands, 65 bytes for Music mode).
    fn get_report(&mut self, w_value: u16, length: u16) -> Result<Vec<u8>, TransportError>;

    /// Sleeps for the specified protocol delay (no-op or recorded in unit tests).
    fn sleep(&mut self, duration: Duration);
}

// ============================================================================
// Concrete Hardware Implementation using `nusb`
// ============================================================================

pub struct NusbTransport {
    interface: nusb::Interface,
    timeout: Duration,
}

impl NusbTransport {
    /// Locates the MSI Per-Key RGB Keyboard Controller (`0x1462:0x1565`) and claims Interface 0.
    pub fn open() -> Result<Self, TransportError> {
        Self::open_with_timeout(USB_CONTROL_TIMEOUT)
    }

    pub fn open_with_timeout(timeout: Duration) -> Result<Self, TransportError> {
        let devices = nusb::list_devices().wait()?;
        let device_info = devices
            .into_iter()
            .find(|d| d.vendor_id() == VENDOR_ID && d.product_id() == PRODUCT_ID)
            .ok_or(TransportError::DeviceNotFound(VENDOR_ID, PRODUCT_ID))?;

        let device = device_info.open().wait()?;
        #[cfg(target_os = "linux")]
        let interface = device.detach_and_claim_interface(INTERFACE_NUMBER).wait()?;
        #[cfg(not(target_os = "linux"))]
        let interface = device.claim_interface(INTERFACE_NUMBER).wait()?;

        Ok(Self { interface, timeout })
    }
}

impl HidControlTransport for NusbTransport {
    fn set_idle(&mut self) -> Result<(), TransportError> {
        self.interface
            .control_out(
                ControlOut {
                    control_type: ControlType::Class,
                    recipient: Recipient::Interface,
                    request: HID_REQ_SET_IDLE,
                    value: WVALUE_SET_IDLE_ALL,
                    index: WINDEX_INTERFACE_0,
                    data: &[],
                },
                self.timeout,
            )
            .wait()?;
        Ok(())
    }

    fn set_report(
        &mut self,
        w_value: u16,
        payload: &[u8; REPORT_LEN],
    ) -> Result<(), TransportError> {
        self.interface
            .control_out(
                ControlOut {
                    control_type: ControlType::Class,
                    recipient: Recipient::Interface,
                    request: HID_REQ_SET_REPORT,
                    value: w_value,
                    index: WINDEX_INTERFACE_0,
                    data: payload,
                },
                self.timeout,
            )
            .wait()?;
        Ok(())
    }

    fn get_report(&mut self, w_value: u16, length: u16) -> Result<Vec<u8>, TransportError> {
        let response = self
            .interface
            .control_in(
                ControlIn {
                    control_type: ControlType::Class,
                    recipient: Recipient::Interface,
                    request: HID_REQ_GET_REPORT,
                    value: w_value,
                    index: WINDEX_INTERFACE_0,
                    length,
                },
                self.timeout,
            )
            .wait()?;

        let expected = usize::from(length);
        if response.len() != expected {
            return Err(TransportError::ReadbackLengthMismatch {
                expected,
                actual: response.len(),
            });
        }
        Ok(response)
    }

    fn sleep(&mut self, duration: Duration) {
        thread::sleep(duration);
    }
}

// ============================================================================
// Mock Transport for Unit Testing
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordedOperation {
    SetIdle,
    SetReport {
        w_value: u16,
        payload: [u8; REPORT_LEN],
    },
    GetReport {
        w_value: u16,
        length: u16,
    },
    Sleep(Duration),
}

#[derive(Debug, Default)]
pub struct MockTransport {
    pub operations: Vec<RecordedOperation>,
    pub canned_get_responses: VecDeque<Vec<u8>>,
    pub fail_next_transfer: Option<String>,
}

impl MockTransport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns only the recorded `SET_REPORT` payloads and their `wValue` targets.
    pub fn set_reports(&self) -> Vec<(u16, [u8; REPORT_LEN])> {
        self.operations
            .iter()
            .filter_map(|op| match op {
                RecordedOperation::SetReport { w_value, payload } => Some((*w_value, *payload)),
                _ => None,
            })
            .collect()
    }
}

impl HidControlTransport for MockTransport {
    fn set_idle(&mut self) -> Result<(), TransportError> {
        if let Some(err) = self.fail_next_transfer.take() {
            return Err(TransportError::Mock(err));
        }
        self.operations.push(RecordedOperation::SetIdle);
        Ok(())
    }

    fn set_report(
        &mut self,
        w_value: u16,
        payload: &[u8; REPORT_LEN],
    ) -> Result<(), TransportError> {
        if let Some(err) = self.fail_next_transfer.take() {
            return Err(TransportError::Mock(err));
        }
        self.operations.push(RecordedOperation::SetReport {
            w_value,
            payload: *payload,
        });
        Ok(())
    }

    fn get_report(&mut self, w_value: u16, length: u16) -> Result<Vec<u8>, TransportError> {
        if let Some(err) = self.fail_next_transfer.take() {
            return Err(TransportError::Mock(err));
        }
        self.operations
            .push(RecordedOperation::GetReport { w_value, length });

        if let Some(resp) = self.canned_get_responses.pop_front() {
            let expected = usize::from(length);
            if resp.len() != expected {
                return Err(TransportError::ReadbackLengthMismatch {
                    expected,
                    actual: resp.len(),
                });
            }
            Ok(resp)
        } else {
            Ok(vec![0u8; usize::from(length)])
        }
    }

    fn sleep(&mut self, duration: Duration) {
        self.operations.push(RecordedOperation::Sleep(duration));
    }
}
