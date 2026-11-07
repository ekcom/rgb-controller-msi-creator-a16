# RGB Keyboard Controller for MSI Creator A16 AI+

A userspace USB HID controller for the **MSI Creator A16 AI+** laptop's per-key RGB keyboard controller.

Contains a library and a CLI driver.

## Requirements

### Supported Devices

- MSI Creator A16 AI+
    - Verified on BIOS release `E15FKAMS.321` (09/17/2026)
    - Other MSI `MS_1565` keyboards may share this controller but are not guaranteed to work

### Dependencies

- cargo

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
