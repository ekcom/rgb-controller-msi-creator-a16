# RGB Keyboard Controller for MSI Creator A16 AI+

## Requirements

### Supported devices

- MSI Creator A16 AI+
    - Other `MS_1565` devices are not guaranteed to work
    - Tested on BIOS version `E15FKAMS.321` (09/17/2026)

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
