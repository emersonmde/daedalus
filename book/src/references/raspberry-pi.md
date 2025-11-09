# Raspberry Pi Documentation

Raspberry Pi 4 specific documentation and resources.

## Primary References

### BCM2711 ARM Peripherals
[BCM2711 ARM Peripherals PDF](https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf)

Complete peripheral reference for the BCM2711 SoC used in Pi 4.

**Key sections:**
- **Section 1.2**: Address map and MMIO base (`0xFE000000` for ARM access)
- **Section 2**: UART (PL011, mini UART)
  - 2.1: PL011 UART registers and configuration
- **Section 5**: GPIO
  - 5.2: Function select and pull-up/down configuration
- **Section 6**: Interrupts (GIC-400)
  - 6.1: GIC distributor base address (`0xFF841000`)
- **Section 10**: System Timer
  - 10.2: System timer registers at `0xFE003000`

**Important notes:**
- Bus addresses in documentation (`0x7E...`) must be translated to ARM physical (`0xFE...`)
- Pi 4 MMIO base changed from Pi 3's `0x3F000000` to `0xFE000000`
- Clock frequencies differ from Pi 3 (e.g., PL011 UART: 54 MHz vs 48 MHz)

### Pi 4 Schematics
[Raspberry Pi 4 Reduced Schematics](https://datasheets.raspberrypi.com/rpi4/raspberry-pi-4-reduced-schematics.pdf)

Hardware schematics showing:
- Power supply routing
- GPIO pin connections
- UART pin assignments (GPIO 14/15 for TXD/RXD)
- Component placement

### Device Tree Reference
[Raspberry Pi Device Tree Documentation](https://www.raspberrypi.com/documentation/computers/configuration.html#device-trees-overlays-and-parameters)

Device tree overlays and parameters for:
- Enabling/disabling peripherals
- UART configuration
- GPIO function assignment

Useful for understanding hardware defaults and firmware configuration.

## Boot Configuration

### config.txt Settings

For bare-metal kernel deployment to SD card:

```ini
enable_uart=1        # Enable PL011 UART for serial console
arm_64bit=1          # Boot in AArch64 mode (required)
kernel=kernel8.img   # Kernel binary to load
```

### Boot Process

1. GPU firmware (start4.elf) loads from SD card FAT partition
2. Firmware initializes hardware and reads config.txt
3. Firmware loads kernel8.img to 0x00080000
4. Firmware jumps to kernel entry point
5. Kernel runs in EL1 (supervisor mode)

See [Boot Sequence](../architecture/boot-sequence.md) for kernel-side boot flow.

## Hardware Differences vs Pi 3

| Feature | Pi 3 (BCM2837) | Pi 4 (BCM2711) |
|---------|----------------|----------------|
| MMIO Base (ARM) | `0x3F000000` | `0xFE000000` |
| UART Clock | 48 MHz | 54 MHz |
| Interrupt Controller | ARM Local | GIC-400 |
| Max RAM | 1 GB | 1/2/4/8 GB |
| USB | 4x USB 2.0 | 2x USB 2.0 + 2x USB 3.0 |

**Code porting note**: Always use memory-map constants, never hardcode Pi 3 addresses.

## QEMU Emulation

### raspi4b Machine Type

[QEMU Raspberry Pi Documentation](https://www.qemu.org/docs/master/system/arm/raspi.html)

- **QEMU 9.0+ required** for raspi4b machine type
- Emulates: CPU, RAM, UART, GPIO (partial), system timer
- Not emulated: PCI, Ethernet, WiFi, USB, GPU

### QEMU vs Real Hardware

| Aspect | QEMU | Real Hardware |
|--------|------|---------------|
| Boot exception level | EL2 (hypervisor) | EL1 (kernel) |
| UART initialization | Pre-configured | Must initialize |
| Timing | Approximate | Cycle-accurate |
| Interrupts | Basic GIC | Full GIC-400 |

See [ADR-002](../decisions/adr-002-qemu-9.md) for QEMU version requirements.

## Useful Resources

- [Raspberry Pi OS Documentation](https://www.raspberrypi.com/documentation/) - Official docs
- [BCM2711 Datasheet](https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf) - Primary peripheral reference
- [Device Tree Source](https://github.com/raspberrypi/linux/tree/rpi-6.6.y/arch/arm64/boot/dts/broadcom) - Kernel device trees (shows hardware configuration)

## Related Documentation

- [Memory Map](../hardware/memory-map.md) - MMIO base addresses
- [UART PL011](../hardware/uart-pl011.md) - UART hardware details
- [ARM Documentation](arm.md) - ARM architecture references
