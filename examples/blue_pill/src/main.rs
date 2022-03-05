//! Example of the aht20-driver crate - displays temperature and humidity
//!
//! This needs an AHT20 temperature and humidity sensor connected to PB6 and PB7. If you're using a
//! Stemma QT/Qwiic cable - the yellow wire should be connected to pb6 (SCL), and the blue wire
//! should be connected to pb7 (SDA).
#![no_main]
#![no_std]

// Global logger using "Real Time Terminal" support for the debugger. I'm using an ST-Link V2 clone.
// This goes via Knurlin-rs' defmt.
use defmt_rtt as _;
use panic_probe as _;

// The Blue Pill's HAL crate imports.
use cortex_m_rt::entry;
use embedded_hal::digital::v2::OutputPin;
use stm32f1xx_hal::{delay, i2c, pac, prelude::*};

use aht20_driver;

const START_TIMEOUT_US: u32 = 10000;
const START_RETRIES: u8 = 5;
const ADDR_TIMEOUT_US: u32 = 10000;
const DATA_TIMEOUT_US: u32 = 10000;


#[entry]
fn main() -> ! {
    // Get access to the core peripherals from the cortex-m crate
    let cp = cortex_m::Peripherals::take().unwrap();
    // Get access to the device specific peripherals from the peripheral access crate
    let dp = pac::Peripherals::take().unwrap();

    // Take ownership over the raw flash and rcc devices and convert them into the corresponding
    // HAL structs
    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.constrain();

    // Freeze the configuration of all the clocks in the system and store the frozen frequencies in
    // `clocks`
    let clocks = rcc.cfgr.freeze(&mut flash.acr);
    let mut delay = delay::Delay::new(cp.SYST, clocks);

    // Acquire the GPIOC peripheral
    let mut gpiob = dp.GPIOB.split(&mut rcc.apb2);
    let mut gpioc = dp.GPIOC.split(&mut rcc.apb2);

    // Configure gpio C pin 13 as a push-pull output. The `crh` register is passed to the function
    // in order to configure the port. For pins 0-7, crl should be passed instead.
    let mut led = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);

    // Set up I2C
    let afio = dp.AFIO.constrain(&mut rcc.apb2);
    let mut mapr = afio.mapr;
    let mut apb = rcc.apb1;
    let scl = gpiob.pb6.into_alternate_open_drain(&mut gpiob.crl);
    let sda = gpiob.pb7.into_alternate_open_drain(&mut gpiob.crl);

    let i2c = i2c::BlockingI2c::i2c1(
        dp.I2C1,
        (scl, sda),
        &mut mapr,
        i2c::Mode::Standard { frequency: 40.hz() },
        clocks,
        &mut apb,
        START_TIMEOUT_US,
        START_RETRIES,
        ADDR_TIMEOUT_US,
        DATA_TIMEOUT_US,
    );

    // Configure the AHT20 temperature and humidity sensor.
    let mut aht20_uninit = aht20_driver::AHT20::new(i2c, aht20_driver::SENSOR_ADDRESS);
    let mut aht20 = aht20_uninit.init(&mut delay).unwrap();

    loop {
        // Take the temperature and humidity measurement.
        let aht20_measurement = aht20.measure(&mut delay).unwrap();
        // NOTE: If you were on a microcontroller with no floating point, then calling
        //       measure_no_fp would keep floating point emulation functions from being
        //       added to the binary, reducing its size and making it faster.
        // let aht20_measurement = aht20.measure_no_fp(&mut delay).unwrap();

        defmt::println!("temperature (aht20): {=f32}C", aht20_measurement.temperature);
        defmt::println!("humidity (aht20): {=f32}%", aht20_measurement.humidity);
        defmt::println!("--");

        // Blink the Blue Pill's onboard LED to show liveness.
        delay.delay_ms(1_000_u16);
        led.set_high().unwrap();
        delay.delay_ms(1_000_u16);
        led.set_low().unwrap();
    }
}
