//! # AHT20 thermometer.
//!
//! This will output a temperature and humidity reading via `defmt`. Modified from the Pico Blinky
//! example, also thus also blinks an LED attached to GP25 (the Pico on-board LED).
//!
//! Run with `DEFMT_LOG=info cargo run`.
//!
//! See the `Cargo.toml` file for Copyright and license details.
#![no_std]
#![no_main]

// For logging via defmt
use defmt_rtt as _;

// The macro for our start-up function
use rp_pico::entry;

// GPIO traits
use embedded_hal::digital::OutputPin;

// Ensure we halt the program on panic (if we don't mention this crate it won't
// be linked)
use panic_halt as _;

// Time handling traits:
use embedded_hal::delay::DelayNs;
use fugit::RateExtU32;

// A shorter alias for the Peripheral Access Crate, which provides low-level
// register access
use rp_pico::hal::pac;

// A shorter alias for the Hardware Abstraction Layer, which provides
// higher-level drivers.
use rp_pico::hal;

use aht20_driver;

/// Entry point to our bare-metal application.
///
/// The `#[entry]` macro ensures the Cortex-M start-up code calls this function
/// as soon as all global variables are initialised.
///
/// The function configures the RP2040 peripherals, then blinks the LED in an
/// infinite loop. It also outputs a temperature reading from the AHT20 that
/// is attached via i2c.
#[entry]
fn main() -> ! {
    // Grab our singleton objects
    let mut pac = pac::Peripherals::take().unwrap();

    // Set up the watchdog driver - needed by the clock setup code
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);

    // Configure the clocks
    //
    // The default is to generate a 125 MHz system clock
    let clocks = hal::clocks::init_clocks_and_plls(
        rp_pico::XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    // The timer object lets us wait for specified amounts of time, and is used by the AHT20
    // driver.
    let mut timer = hal::Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    // The single-cycle I/O block controls our GPIO pins
    let sio = hal::Sio::new(pac.SIO);

    // Set the pins up according to their function on this particular board
    let pins = rp_pico::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // Set the LED to be an output
    let mut led_pin = pins.led.into_push_pull_output();
    // Configure two pins as being I²C, not GPIO, for the AHT20.
    let sda_pin: hal::gpio::Pin<_, hal::gpio::FunctionI2C, _> = pins.gpio16.reconfigure();
    let scl_pin: hal::gpio::Pin<_, hal::gpio::FunctionI2C, _> = pins.gpio17.reconfigure();

    // Create the I²C driver, using the two pre-configured pins. This will fail
    // at compile time if the pins are in the wrong mode, or if this I²C
    // peripheral isn't available on these pins!
    let i2c = hal::I2C::i2c0(
        pac.I2C0,
        sda_pin,
        scl_pin,
        400.kHz(),
        &mut pac.RESETS,
        &clocks.peripheral_clock,
    );

    // Configure the AHT20 temperature and humidity sensor.
    let mut aht20_uninit = aht20_driver::AHT20::new(i2c, aht20_driver::SENSOR_ADDRESS);
    let mut aht20 = aht20_uninit.init(&mut timer).unwrap();
    defmt::info!("setup done");

    // Blink the LED at 1 Hz and get temperature and humidity readings.
    loop {
        led_pin.set_high().unwrap();
        timer.delay_ms(500);

        // Take the temperature and humidity measurement.
        let aht20_measurement = aht20.measure(&mut timer).unwrap();

        defmt::info!("temperature: {}", aht20_measurement.temperature);
        defmt::info!("humidity: {}", aht20_measurement.humidity);

        led_pin.set_low().unwrap();
        timer.delay_ms(500);
    }
}
