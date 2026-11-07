# RGB Keyboard Controller for MSI Creator A16 AI+

## Requirements

### Supported devices

- MSI Creator A16 AI+
    - Other `MS_1565` devices are not guaranteed to work
    - Tested on BIOS version `E15FKAMS.321` (09/17/2026)

### Dependencies

- cargo

## Installation

### 1. Configure udev rule

In order to access the device without root, you must configure a udev rule to grant access.

Create `/etc/udev/rules.d/60-rgb-controller-msi-creator-a16.rules`:

```
SUBSYSTEMS=="hidraw", ATTRS{idVendor}=="1462", ATTRS{idProduct}=="1565", TAG+="uaccess"
```

### 2. Build & run program

```
cargo run
```
