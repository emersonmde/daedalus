//! BCM2711 GPIO driver for general-purpose digital I/O.
//!
//! Provides configuration and control of the Raspberry Pi 4's GPIO pins.
//! Supports pin mode selection (input/output/alt functions), pull-up/down
//! resistor configuration, and digital I/O operations.
//!
//! **IMPORTANT**: BCM2711 uses different pull-up/down registers than BCM2835!
//! The old GPPUD/GPPUDCLK registers are not connected on BCM2711.

use volatile::Volatile;

/// GPIO base address (BCM2711 ARM physical address mapping).
///
/// Source: BCM2711 Peripherals Section 5
/// <https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf>
const GPIO_BASE: usize = 0xFE20_0000;

/// Number of GPIO pins available on BCM2711.
///
/// BCM2711 has 58 GPIO pins (0-57), an increase from 54 on BCM2835.
const NUM_GPIOS: u32 = 58;

/// GPIO register offsets from GPIO_BASE.
///
/// Reference: BCM2711 Peripherals Section 5, and Linux kernel driver:
/// <https://github.com/torvalds/linux/blob/master/drivers/pinctrl/bcm/pinctrl-bcm2835.c>
#[allow(dead_code)]
mod offsets {
    // Function Select registers (6 registers, 10 pins each, 3 bits per pin)
    pub const GPFSEL0: usize = 0x00;
    pub const GPFSEL1: usize = 0x04;
    pub const GPFSEL2: usize = 0x08;
    pub const GPFSEL3: usize = 0x0C;
    pub const GPFSEL4: usize = 0x10;
    pub const GPFSEL5: usize = 0x14;

    // Output Set registers (2 registers, 32 pins each)
    pub const GPSET0: usize = 0x1C;
    pub const GPSET1: usize = 0x20;

    // Output Clear registers (2 registers, 32 pins each)
    pub const GPCLR0: usize = 0x28;
    pub const GPCLR1: usize = 0x2C;

    // Pin Level registers (2 registers, 32 pins each, read-only)
    pub const GPLEV0: usize = 0x34;
    pub const GPLEV1: usize = 0x38;

    // Pull-up/down control registers (BCM2711 specific!)
    // 4 registers, 16 pins each, 2 bits per pin
    pub const GPIO_PUP_PDN_CNTRL_REG0: usize = 0xE4;
    pub const GPIO_PUP_PDN_CNTRL_REG1: usize = 0xE8;
    pub const GPIO_PUP_PDN_CNTRL_REG2: usize = 0xEC;
    pub const GPIO_PUP_PDN_CNTRL_REG3: usize = 0xF0;
}

/// GPIO pin function modes.
///
/// Each pin can be configured as input, output, or one of six alternate
/// functions for hardware peripherals (UART, SPI, I2C, PWM, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Function {
    Input = 0b000,
    Output = 0b001,
    Alt0 = 0b100,
    Alt1 = 0b101,
    Alt2 = 0b110,
    Alt3 = 0b111,
    Alt4 = 0b011,
    Alt5 = 0b010,
}

/// GPIO pin pull-up/down resistor modes (BCM2711).
///
/// Controls internal pull resistors (~50-60kΩ) on input pins.
/// On BCM2711, these are configured via GPIO_PUP_PDN_CNTRL_REGx registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Pull {
    None = 0b00,
    Up = 0b01,
    Down = 0b10,
}

/// GPIO controller for BCM2711.
///
/// Provides safe access to GPIO pin configuration and I/O operations.
pub struct Gpio {
    base: usize,
}

impl Gpio {
    /// Create a new GPIO controller instance.
    ///
    /// # Safety
    ///
    /// This function is safe because:
    /// 1. GPIO_BASE (0xFE200000) is the documented GPIO base address for BCM2711
    ///    - BCM2711 peripherals start at 0xFE000000 (Low Peripheral Mode)
    ///    - GPIO offset is 0x200000 from peripheral base
    /// 2. This address is reserved by hardware and always accessible
    /// 3. All register accesses use volatile reads/writes
    pub const fn new() -> Self {
        Gpio { base: GPIO_BASE }
    }

    /// Set the function mode of a GPIO pin.
    ///
    /// Configures the pin as input, output, or one of the alternate functions.
    ///
    /// # Arguments
    ///
    /// * `pin` - GPIO pin number (0-57)
    /// * `function` - Desired function mode
    ///
    /// # Panics
    ///
    /// Panics if pin >= 58.
    pub fn set_function(&self, pin: u32, function: Function) {
        assert!(pin < NUM_GPIOS, "GPIO pin {} out of range (0-57)", pin);

        // Each GPFSEL register controls 10 pins, 3 bits each
        let reg_index = (pin / 10) as usize;
        let bit_offset = (pin % 10) * 3;
        let mask = 0b111 << bit_offset;
        let value = (function as u32) << bit_offset;

        // Calculate register address
        let reg_addr = self.base + offsets::GPFSEL0 + (reg_index * 4);

        // SAFETY: Register address is within GPIO peripheral range and properly aligned.
        // Volatile read-modify-write ensures proper hardware access.
        unsafe {
            let reg = reg_addr as *mut Volatile<u32>;
            let current = (*reg).read();
            (*reg).write((current & !mask) | value);
        }
    }

    /// Configure pull-up/down resistor for a GPIO pin (BCM2711 specific).
    ///
    /// Sets the internal pull resistor mode for the specified pin.
    ///
    /// # Arguments
    ///
    /// * `pin` - GPIO pin number (0-57)
    /// * `pull` - Desired pull resistor mode
    ///
    /// # Panics
    ///
    /// Panics if pin >= 58.
    pub fn set_pull(&self, pin: u32, pull: Pull) {
        assert!(pin < NUM_GPIOS, "GPIO pin {} out of range (0-57)", pin);

        // Each GPIO_PUP_PDN_CNTRL_REG controls 16 pins, 2 bits each
        let reg_index = (pin / 16) as usize;
        let bit_offset = (pin % 16) * 2;
        let mask = 0b11 << bit_offset;
        let value = (pull as u32) << bit_offset;

        // Calculate register address
        let reg_addr = self.base + offsets::GPIO_PUP_PDN_CNTRL_REG0 + (reg_index * 4);

        // SAFETY: Register address is within GPIO peripheral range and properly aligned.
        // Volatile read-modify-write ensures proper hardware access.
        unsafe {
            let reg = reg_addr as *mut Volatile<u32>;
            let current = (*reg).read();
            (*reg).write((current & !mask) | value);
        }
    }

    /// Set a GPIO output pin HIGH.
    ///
    /// Writes 1 to the GPSET register to set the pin output high (3.3V).
    /// Only affects pins configured as outputs. Writing 0 has no effect.
    ///
    /// # Arguments
    ///
    /// * `pin` - GPIO pin number (0-57)
    ///
    /// # Panics
    ///
    /// Panics if pin >= 58.
    pub fn set(&self, pin: u32) {
        assert!(pin < NUM_GPIOS, "GPIO pin {} out of range (0-57)", pin);

        // GPSET0/1 registers: write 1 to set pin high
        let reg_index = (pin / 32) as usize;
        let bit = 1 << (pin % 32);
        let reg_addr = self.base + offsets::GPSET0 + (reg_index * 4);

        // SAFETY: Register address is within GPIO peripheral range and properly aligned.
        // Write-only register (reads return 0), writing 1 sets pin high.
        unsafe {
            let reg = reg_addr as *mut Volatile<u32>;
            (*reg).write(bit);
        }
    }

    /// Set a GPIO output pin LOW.
    ///
    /// Writes 1 to the GPCLR register to set the pin output low (0V).
    /// Only affects pins configured as outputs. Writing 0 has no effect.
    ///
    /// # Arguments
    ///
    /// * `pin` - GPIO pin number (0-57)
    ///
    /// # Panics
    ///
    /// Panics if pin >= 58.
    pub fn clear(&self, pin: u32) {
        assert!(pin < NUM_GPIOS, "GPIO pin {} out of range (0-57)", pin);

        // GPCLR0/1 registers: write 1 to set pin low
        let reg_index = (pin / 32) as usize;
        let bit = 1 << (pin % 32);
        let reg_addr = self.base + offsets::GPCLR0 + (reg_index * 4);

        // SAFETY: Register address is within GPIO peripheral range and properly aligned.
        // Write-only register (reads return 0), writing 1 sets pin low.
        unsafe {
            let reg = reg_addr as *mut Volatile<u32>;
            (*reg).write(bit);
        }
    }

    /// Read the current level of a GPIO pin.
    ///
    /// Returns the actual voltage level on the pin (true = high, false = low),
    /// regardless of whether the pin is configured as input or output.
    ///
    /// # Arguments
    ///
    /// * `pin` - GPIO pin number (0-57)
    ///
    /// # Returns
    ///
    /// `true` if pin is high (3.3V), `false` if low (0V).
    ///
    /// # Panics
    ///
    /// Panics if pin >= 58.
    pub fn read(&self, pin: u32) -> bool {
        assert!(pin < NUM_GPIOS, "GPIO pin {} out of range (0-57)", pin);

        // GPLEV0/1 registers: read current pin level
        let reg_index = (pin / 32) as usize;
        let bit = 1 << (pin % 32);
        let reg_addr = self.base + offsets::GPLEV0 + (reg_index * 4);

        // SAFETY: Register address is within GPIO peripheral range and properly aligned.
        // Read-only register, returns actual pin voltage level.
        unsafe {
            let reg = reg_addr as *mut Volatile<u32>;
            ((*reg).read() & bit) != 0
        }
    }

    /// Toggle a GPIO output pin.
    ///
    /// Reads the current pin level and writes the opposite value.
    ///
    /// # Arguments
    ///
    /// * `pin` - GPIO pin number (0-57)
    ///
    /// # Panics
    ///
    /// Panics if pin >= 58.
    pub fn toggle(&self, pin: u32) {
        if self.read(pin) {
            self.clear(pin);
        } else {
            self.set(pin);
        }
    }

    /// Write a boolean value to a GPIO output pin.
    ///
    /// Convenience method: `true` sets pin high, `false` sets pin low.
    ///
    /// # Arguments
    ///
    /// * `pin` - GPIO pin number (0-57)
    /// * `value` - Value to write (true = high, false = low)
    ///
    /// # Panics
    ///
    /// Panics if pin >= 58.
    pub fn write(&self, pin: u32, value: bool) {
        if value {
            self.set(pin);
        } else {
            self.clear(pin);
        }
    }
}

impl Default for Gpio {
    fn default() -> Self {
        Self::new()
    }
}
