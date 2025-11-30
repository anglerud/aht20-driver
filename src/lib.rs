#![cfg_attr(not(test), no_std)]
//! AHT20 driver.
//!
//! Example:
//!
//!     # use embedded_hal_mock::eh1::delay::NoopDelay as MockDelay;
//!     # use embedded_hal_mock::eh1::i2c::Mock as I2cMock;
//!     # use embedded_hal_mock::eh1::i2c::Transaction;
//!     # use aht20_driver::{AHT20, AHT20Initialized, Command, SENSOR_ADDRESS};
//!     # let expectations = vec![
//!     #     // check_status immediately succeeds, we don't need to send Initialize.
//!     #     Transaction::read(SENSOR_ADDRESS, vec![0b0000_1000]),
//!     #     // send_trigger_measurement
//!     #     Transaction::write(
//!     #         SENSOR_ADDRESS,
//!     #         vec![
//!     #             Command::TriggerMeasurement as u8,
//!     #             0b0011_0011, // 0x33
//!     #             0b0000_0000, // 0x00
//!     #         ],
//!     #     ),
//!     #     // check_status - with ready bit set to 'ready' (off)
//!     #     Transaction::read(SENSOR_ADDRESS, vec![0b0000_1000]),
//!     #     // We can now read 7 bytes. status byte, 5 data bytes, crc byte.
//!     #     // These are taken from a run of the sensor.
//!     #     Transaction::read(
//!     #         SENSOR_ADDRESS,
//!     #         vec![
//!     #             0b0001_1100, //  28, 0x1c - ready, calibrated, and some mystery flags.
//!     #             //             bit 8 set to 0 is ready. bit 4 set is calibrated. bit 5
//!     #             //             and 3 are described as 'reserved'.
//!     #             0b0110_0101, // 101, 0x65 - first byte of humidity value
//!     #             0b1011_0100, // 180, 0xb4 - second byte of humidity vaue
//!     #             0b0010_0101, //  37, 0x25 - split byte. 4 bits humidity, 4 bits temperature.
//!     #             0b1100_1101, // 205, 0xcd - first full byte of temperature.
//!     #             0b0010_0110, //  38, 0x26 - second full byte of temperature.
//!     #             0b1100_0110, // 198, 0xc6 - CRC
//!     #         ],
//!     #     ),
//!     # ];
//!     # let mock_i2c = I2cMock::new(&expectations);
//!     # let mut mock_delay = MockDelay::new();
//!     let aht20_uninit = AHT20::new(mock_i2c, SENSOR_ADDRESS);
//!     let mut aht20 = aht20_uninit.init(&mut mock_delay).unwrap();
//!     let measurement = aht20.measure(&mut mock_delay).unwrap();
//!
//!     println!("temperature (aht20): {:.2}C", measurement.temperature);
//!     println!("humidity (aht20): {:.2}%", measurement.humidity);
//!
//!     aht20.destroy().destroy().done();
//!
//! [AHT20 Datasheet](https://cdn-learn.adafruit.com/assets/assets/000/091/676/original/AHT20-datasheet-2020-4-16.pdf?1591047915)
//!
//! Note that the datasheet linked directly from the manufacturer's website
//! [Aogong AHT20](http://www.aosong.com/en/products-32.html) is an older datasheet (version
//! 1.0, rather than version 1.1 as linked above) and is significantly more
//! difficult to understand. I recommend reading version 1.1. All section
//! references in this file are to the 1.1 version.
//!
//! The below is a flowchart of how the sensor gets initialized and measurements taken.
//! Note that the flowchart does not include the parameters that you need to give to
//! some commands, and it also doesn't include the SoftReset command flow.
//!
//! ```text
//!           Start (Power on)
//!                  │
//!                  ▼
//!              Wait 40 ms
//!                  │
//!                  ▼
//!           Read status byte    ◄───     Wait 10 ms
//!                  │                           ▲
//!                  ▼                           │
//!          Status::Calibrated ──► No ──► Command::Initialize (0xBE)
//!                  │
//!                  ▼
//!                 Yes
//!                  │
//!                  ▼
//! Command::TriggerMeasurement  (0xAC)  ◄─┐
//!                  │                     │
//!                  ▼                     │
//!             Wait 80 ms                 │
//!                  │                     │
//!                  ▼                     │
//!           Read status byte    ◄──┐     │
//!                  │               │     │
//!                  ▼               │     │
//!             Status::Busy  ───►  Yes    │
//!                  │                     │
//!                  ▼                     │
//!                 No                     │
//!                  │                     │
//!                  ▼                     │
//!             Read 7 bytes               │
//!                  │                     │
//!                  ▼                     │
//!             Calculate CRC              │
//!                  │                     │
//!                  ▼                     │
//!               CRC good ─► No  ─────────┘
//!                  │                     ▲
//!                  ▼                     │
//!                 Yes                    │
//!                  │                     │
//!                  ▼                     │
//!           CRC-checked Ready ─► No ─────┘
//!                  │
//!                  ▼
//!                 Yes
//!                  │
//!                  ▼
//!        Calc Humidity and Temp
//! ```

use crc_any::CRCu8;
use embedded_hal::delay::DelayNs;
use embedded_hal::i2c::I2c;

/// AHT20 sensor's I2C address.
pub const SENSOR_ADDRESS: u8 = 0b0011_1000; // This is I2C address 0x38;

/// Commands that can be sent to the AHT20 sensor.
///
/// Note that a few of these take parameters but that there are no explanations provided about what
/// those parameters actually are. You should consider the command and specified parameters to be
/// just one three-byte command. These can be found in the datasheet, Section 5.3, page 8, Table 9.
pub enum Command {
    Initialize = 0b1011_1110, // 0xBE, Initialize and calibrate the sensor.
    // This command takes two bytes of parameter: 0b0000_1000 (0x08), then 0b0000_0000 (0x00).
    Calibrate = 0b1110_0001, // 0xE1, Calibrate - or return calibration status.
    // Status will be Status::Calibrated, where bit4 indicates calibrated. If it's 0, it's not.
    TriggerMeasurement = 0b1010_1100, // 0xAC
    // This command takes two bytes of parameter: 0b00110011 (0x33), then 0b0000_0000 (0x00).
    // Wait 80ms for the measurement. You'll get a status byte back. Check the status for
    // Status::Busy to be 0. If it is, then read 7 bytes. A status byte, 5 data plus a byte of CRC.
    SoftReset = 0b1011_1010, // 0xBA
                             // Also see Section 5.5. This takes 20ms or less to complete.
}

/// Status byte meanings.
///
/// Table 10, page 8 of the datasheet.
pub enum Status {
    Busy = 0b1000_0000, // Status bit for busy - 8th bit enabled. 1<<7, 0x80
    // 1 is Busy measuring. 0 is "Free in dormant state" or "ready".
    Calibrated = 0b0000_1000, // Status bit for calibrated - 4th bit enabled. 1<<3, 0x08.
                              // 1 is Calibrated, 0 is uncalibrated. If 0, send Command::Initialize.
}

/// SensorStatus is the response from the sensor indicating if it is ready to read from, and if it
/// is calibrated.
///
/// This is returned from the `check_status` method. It is used both
/// during initialization, which is when the sensor caibrates itself, and during
/// measure. During measure the sensor will report itself as busy (not ready)
/// for a period of 80ms.
#[derive(Debug, Clone, Copy)]
pub struct SensorStatus(pub u8);

impl SensorStatus {
    /// Create a new SensorStatus from an AHT20 status byte.
    ///
    /// That byte comes from the `check_status` method.
    pub fn new(status: u8) -> Self {
        SensorStatus(status)
    }

    /// Check if the sensor is ready to have data read from it. After issuing a sensor read, you
    /// must check is_ready before reading the result. The measure function takes care of this wait
    /// and check.
    pub fn is_ready(self) -> bool {
        // The busy bit should be 0 (not busy) for the sensor to report ready.
        (self.0 & Status::Busy as u8) == 0
    }

    /// Check if the sensor is calibrated. If it is not, you must call `init` to initialize the
    /// sensor.
    pub fn is_calibrated(self) -> bool {
        // The calibrated bit should be set.
        (self.0 & Status::Calibrated as u8) != 0
    }
}

/// SensorReading is a single reading from the AHT20 sensor.
///
/// This is returned from the `measure` method. You get:
/// * humidity in % Relative Humidity
/// * temperature in degrees Celsius.
#[derive(Debug, Clone, Copy)]
pub struct SensorReading {
    pub humidity: f32,
    pub temperature: f32,
}

impl SensorReading {
    /// Create a SensorReading from the data returned by the sensor.
    ///
    /// This is done by the `measure` method.
    fn from_bytes(sensor_data: [u8; 5]) -> Self {
        let (humidity_val, temperature_val) = SensorReading::raw_from_bytes(sensor_data);

        // From section 6.1 "Relative humidity transformation" here is how we turn this into
        // a relative humidity percantage value.
        let humidity_percent = (humidity_val as f32) / ((1 << 20) as f32) * 100.0;

        // From section 6.2 "Temperature transformation" here is how we turn this into
        // a temprature in °C.
        let temperature_celcius = (temperature_val as f32) / ((1 << 20) as f32) * 200.0 - 50.0;

        SensorReading {
            humidity: humidity_percent,
            temperature: temperature_celcius,
        }
    }

    /// Identical to `from_bytes`, but doesn't use floating point math.
    ///
    /// This limits the precision to just integer values, but doesn't bring in floating point
    /// libraries on microcontrollers with no FP support, saving space and being faster.
    fn from_bytes_no_fp(sensor_data: [u8; 5]) -> Self {
        let (humidity_val, temperature_val) = SensorReading::raw_from_bytes(sensor_data);

        // From section 6.1 "Relative humidity transformation" here is how we turn this into
        // a relative humidity percantage value.
        let humidity_percent = (100 * humidity_val) >> 20;

        // From section 6.2 "Temperature transformation" here is how we turn this into
        // a temprature in °C.
        let temperature_celcius = (((200 * temperature_val) >> 20) - 50) as f32;

        SensorReading {
            humidity: humidity_percent as f32,
            temperature: temperature_celcius,
        }
    }

    /// Turn the raw data from a sensor reding into two u32 raw values.
    ///
    /// These values still need to be converted into %age humidity and degrees C, this is done with
    /// `from_bytes` and `from_bytes_no_fp`. This is just a helper method for the aforementioned.
    fn raw_from_bytes(sensor_data: [u8; 5]) -> (u32, u32) {
        // Our five bytes of sensor data is split into 20 bits (two and a half bytes) humidity and
        // 20 bits temperature. We'll have to spend a bit of time splitting the middle byte up.
        let humidity_bytes: &[u8] = &sensor_data[..2];
        let split_byte: u8 = sensor_data[2];
        let temperature_bytes: &[u8] = &sensor_data[3..5];

        // We have a byte that might look like 0x0101_1010, we want only the first four bits, (the
        // 0101) to be at the end of the byte. So we shift them four right and end up with
        // 0x0000_0101. These 4 bits go at the very end of our 20-bit humidity value.
        // In the final 32-bit value they're these ones: 0x0000_0000_0000_0000_0000_0000_0000_1111
        let right_bits_humidity: u32 = (split_byte >> 4).into();
        // In the final 32-bit value they're these ones: 0x0000_0000_0000_1111_1111_0000_0000_0000
        let left_bits_humidity: u32 = (humidity_bytes[0] as u32) << 12;
        // In the final 32-bit value they're these ones: 0x0000_0000_0000_0000_0000_1111_1111_0000
        let middle_bits_humidity: u32 = (humidity_bytes[1] as u32) << 4;
        // We combine them to form the complete 20 bits: 0x0000_0000_0000_1111_1111_1111_1111_1111
        let humidity_val: u32 = left_bits_humidity | middle_bits_humidity | right_bits_humidity;

        // With that same example byte - we want to keep only the last four bits this time, so we
        // mask the first four and end up with 0x0000_1010. These bits end up at the very start of
        // our 20-bit temperature value. In the final 32-bit value they're these ones:
        // 0x0000_0000_0000_1111_0000_0000_0000_0000 To get them into their final position - we'll
        // left-shift them by 16 positions.
        let split_byte_temperature: u32 = (split_byte & 0b0000_1111).into();
        // We need to fill the rightmost 20 bits, starting with our split byte
        // In the final 32-bit value they're these ones: 0x0000_0000_0000_1111_0000_0000_0000_0000
        let left_bits_temp: u32 = split_byte_temperature << 16;
        // In the final 32-bit value they're these ones: 0x0000_0000_0000_0000_1111_1111_0000_0000
        let middle_bits_temp: u32 = (temperature_bytes[0] as u32) << 8;
        // And just for symmetry...
        // In the final 32-bit value they're these ones: 0x0000_0000_0000_0000_0000_0000_1111_1111
        let right_bits_temp: u32 = temperature_bytes[1] as u32;
        // We combine them to form the complete 20 bits: 0x0000_0000_0000_1111_1111_1111_1111_1111
        let temperature_val: u32 = left_bits_temp | middle_bits_temp | right_bits_temp;

        (humidity_val, temperature_val)
    }
}

/// Driver errors.
#[derive(Debug, PartialEq)]
pub enum Error<E> {
    /// I2C bus error
    I2c(E),
    /// CRC validation failed
    InvalidCrc,
    /// Unexpectedly not ready - this can happen when the sensor sends back "busy" but the
    /// I2C data gets corrupted and we receive "ready", then later the
    /// CRC-checked status byte correctly reports "busy" and we have to abort the measurement.
    UnexpectedBusy,
    /// Errors such as overflowing the stack.
    Internal,
}

impl<E> core::fmt::Display for Error<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        match self {
            Error::I2c(_e) => write!(f, "I2C error"),
            Error::InvalidCrc => write!(f, "invalid CRC error"),
            Error::UnexpectedBusy => write!(f, "unexpected busy error"),
            Error::Internal => write!(f, "internal ATH20 driver error"),
        }
    }
}

impl<E> core::error::Error for Error<E> where E: core::fmt::Debug {}

/// An AHT20 sensor on the I2C bus `I`.
///
/// The address of the sensor will be `SENSOR_ADDRESS` from this package, unless there is some kind
/// of special address translating hardware in use.
#[derive(Clone, Copy)]
pub struct AHT20<I>
where
    I: I2c,
{
    i2c: I,
    address: u8,
}

impl<I> AHT20<I>
where
    I: I2c,
{
    /// Initializes the AHT20 driver.
    ///
    /// This consumes the I2C bus `I`. Before you can get temperature and humidity measurements,
    /// you must call the `init` method which calibrates the sensor. The address will almost always
    /// be `SENSOR_ADDRESS` from this crate.
    pub fn new(i2c: I, address: u8) -> Self {
        AHT20 { i2c, address }
    }

    /// Run the AHT20 init and calibration routines.
    ///
    /// This must be called before any other methods except `check_status`. This method will take
    /// *at least* 40ms to return.
    ///
    /// ```text
    ///          Start (Power on)
    ///                 │
    ///                 ▼
    ///             Wait 40 ms
    ///                 │
    ///                 ▼
    ///           Read  status byte    ◄───    Wait 10 ms
    ///                 │                           ▲
    ///                 ▼                           │
    ///         Status::Calibrated ──► No ──► Command::Initialize (0xBE)
    ///                 │
    ///                 ▼
    ///                Yes
    /// ```
    pub fn init(
        mut self,
        delay: &mut impl DelayNs,
    ) -> Result<AHT20Initialized<I>, Error<I::Error>> {
        delay.delay_ms(40);

        while !self.check_status()?.is_calibrated() {
            self.send_initialize()?;
            #[cfg(feature = "use-defmt")]
            defmt::debug!("init: waiting for sensor to report being calibrated, 10ms.");
            delay.delay_ms(10);
        }

        #[cfg(feature = "use-defmt")]
        defmt::debug!("init: sensor reporting being calibrated, init done.");
        Ok(AHT20Initialized { aht20: self })
    }

    /// check_Status reads a status byte from the AHT20 sensor to check its status.
    ///
    /// The sensor can be calibrated or not, also busy generating a sensor measurement or ready.
    /// This method returns the SensorStatus struct, which you can use to determine what the state
    /// of the sensor is.
    ///
    /// NOTE: The documentation suggests that we send a CheckStatus (0x71) command, followed by a
    ///       read. Experience (https://github.com/anglerud/aht20-driver/pull/10) indicates that we
    ///       can create a hang writing that command, and that just reading a status byte works.
    ///
    /// This is used by both measure_once and init.
    fn check_status(&mut self) -> Result<SensorStatus, Error<I::Error>> {
        #[cfg(feature = "use-defmt")]
        defmt::debug!("check_status: requesting a status check from sensor.");
        let mut read_buffer = [0u8; 1];

        self.i2c
            .read(self.address, &mut read_buffer)
            .map_err(Error::I2c)?;

        let status_byte = read_buffer[0];
        Ok(SensorStatus::new(status_byte))
    }

    /// send_initialize sends the Initialize command to the sensor which make it calibrate.
    ///
    /// After sending initialize, there is a required 40ms wait period and verification
    /// that the sensor reports itself calibrated. See the `init` method.
    fn send_initialize(&mut self) -> Result<(), Error<I::Error>> {
        #[cfg(feature = "use-defmt")]
        defmt::debug!("send_initialize: requesting sensor to initialize itself.");
        let command: [u8; 3] = [
            // Initialize = 0b1011_1110. Equivalent to 0xBE, Section 5.3, page 8, Table 9
            Command::Initialize as u8,
            // Two parameters as described in the datasheet. There is no indication what these
            // parameters mean, just that they should be provided. There is also no returned
            // value.
            0b0000_1000, // 0x08
            0b0000_0000, // 0x00
        ];

        self.i2c.write(self.address, &command).map_err(Error::I2c)?;

        Ok(())
    }

    /// Destroys this driver and releases the I2C bus `I`
    pub fn destroy(self) -> I {
        self.i2c
    }
}

/// AHT20Initialized is returned by AHT20::init() and the sensor is ready to read from.
///
/// In this state you can trigger a measurement with `.measure(&mut delay)`.
pub struct AHT20Initialized<I>
where
    I: I2c,
{
    aht20: AHT20<I>,
}

impl<I> AHT20Initialized<I>
where
    I: I2c,
{
    /// Measure temperature and humidity.
    ///
    /// This masurement takes at least 80ms to complete. Together with the `measure_once` method,
    /// this is the work being carried out:
    ///
    /// ```text
    /// Command::TriggerMeasurement (0xAC)   ◄─┐
    ///                  │                     │
    ///                  ▼                     │
    ///             Wait 80 ms                 │
    ///                  │                     │
    ///                  ▼                     │
    ///           Read status byte    ◄──┐     │
    ///                  │               │     │
    ///                  ▼               │     │
    ///             Status::Busy  ───►  Yes    │
    ///                  │                     │
    ///                  ▼                     │
    ///                 No                     │
    ///                  │                     │
    ///                  ▼                     │
    ///             Read 7 bytes               │
    ///                  │                     │
    ///                  ▼                     │
    ///             Calculate CRC              │
    ///                  │                     │
    ///                  ▼                     │
    ///               CRC good ─► No  ─────────┘
    ///                  │                     ▲
    ///                  ▼                     │
    ///                 Yes                    │
    ///                  │                     │
    ///                  ▼                     │
    ///           CRC-checked Ready ─► No ─────┘
    ///                  │
    ///                  ▼
    ///                 Yes
    ///                  │
    ///                  ▼
    ///        Calc Humidity and Temp
    /// ```
    pub fn measure(&mut self, delay: &mut impl DelayNs) -> Result<SensorReading, Error<I::Error>> {
        loop {
            let measurement_result = self.measure_once(delay);
            match measurement_result {
                Ok(sb) => {
                    return Ok(SensorReading::from_bytes(sb));
                }
                Err(Error::InvalidCrc) => {
                    // CRC failed to validate, we'll go back and issue another read request.
                    #[cfg(feature = "use-defmt")]
                    defmt::error!("Invalid CRC, retrying.");
                }
                Err(Error::UnexpectedBusy) => {
                    // Possibly indicates the previously seen 'ready' was due to uncorrected noise.
                    #[cfg(feature = "use-defmt")]
                    defmt::error!("Sensor contradicted a ready status with a crc-checked busy.");
                }
                Err(other) => return Err(other),
            }
        }
    }

    /// This is identical to `measure`, except it doesn't use floating point math.
    ///
    /// This means it can be used on microcontrollers with no FP support, without having
    /// to bring in floating point math functions, which can take up a lot of space, and
    /// might be slow.
    ///
    /// The drawback is that precision is limited to only integer values.
    pub fn measure_no_fp(
        &mut self,
        delay: &mut impl DelayNs,
    ) -> Result<SensorReading, Error<I::Error>> {
        // TODO: See if we can refactor here, the only difference is from_bytes and
        //       from_bytes_no_fp.
        loop {
            let measurement_result = self.measure_once(delay);
            match measurement_result {
                Ok(sb) => {
                    return Ok(SensorReading::from_bytes_no_fp(sb));
                }
                Err(Error::InvalidCrc) => {
                    // CRC failed to validate, we'll go back and issue another read request.
                    #[cfg(feature = "use-defmt")]
                    defmt::error!("Invalid CRC, retrying.");
                }
                Err(Error::UnexpectedBusy) => {
                    // Possibly indicates the previously seen 'ready' was due to uncorrected noise.
                    #[cfg(feature = "use-defmt")]
                    defmt::error!("Sensor contradicted a ready status with a crc-checked busy.");
                }
                Err(other) => return Err(other),
            }
        }
    }

    /// Perform one measurement and return the sensor's 5 raw data bytes.
    ///
    /// This takes at least 80ms to complete, and only returns 2x20 bits in 5 bytes.
    /// This data is interpreted by the `measure` function.
    fn measure_once(&mut self, delay: &mut impl DelayNs) -> Result<[u8; 5], Error<I::Error>> {
        self.send_trigger_measurement()?;
        delay.delay_ms(80);

        // Wait for measurement to be ready
        while !self.aht20.check_status()?.is_ready() {
            #[cfg(feature = "use-defmt")]
            defmt::debug!("measure_once: waiting for ready, 1ms.");
            delay.delay_ms(1);
        }

        // 1 byte status, 20 bits humidity + 20 bits temperature, 1 byte CRC
        let mut read_buffer = [0u8; 7];
        self.aht20
            .i2c
            .read(self.aht20.address, &mut read_buffer)
            .map_err(Error::I2c)?;

        let data: &[u8] = &read_buffer[..6];
        let crc_byte: u8 = read_buffer[6];

        let crc = compute_crc(data);
        if crc_byte != crc {
            return Err(Error::InvalidCrc);
        }

        // The first byte of the sensor's response is a repeat of the status byte.
        // There is a minescule chance that the previous ready message was caused
        // by noise on the i2c bus. This byte has been CRC-checked.
        let status = SensorStatus::new(read_buffer[0]);
        if !status.is_ready() {
            return Err(Error::UnexpectedBusy);
        }

        // Arrays implement TryFrom for slices. In case the length of the slice does not match
        // the requested array - it will return a TryFromSliceError, but we are selecting the
        // right number of bytes so there is no risk. Mapping to a generic error.
        data[1..6].try_into().map_err(|_| Error::Internal)
    }

    /// Send the "Trigger Measurement" command to the sensor.
    ///
    /// This does not return anything, it only instructs the sensor to get the data ready. After
    /// sending this command, you need to wait 80ms before attempting to read data back. See the
    /// `measure_once` function and the flowchart at the top of this file.
    fn send_trigger_measurement(&mut self) -> Result<(), Error<I::Error>> {
        // TriggerMeasurement is 0b1010_1100. Equivalent to 0xAC: Section 5.3, page 8, Table 9
        // This command takes two bytes of parameter:  0b00110011 (0x33), then 0b0000_0000 (0x00).
        let command: [u8; 3] = [
            Command::TriggerMeasurement as u8,
            // Two parameters as described in the datasheet. There is no indication what these
            // parameters mean, just that they should be provided. There is no returned value.
            // To get the measurement, see [measure](measure).
            0b0011_0011, // 0x33
            0b0000_0000, // 0x00
        ];

        self.aht20
            .i2c
            .write(self.aht20.address, &command)
            .map_err(Error::I2c)?;

        Ok(())
    }

    /// Send the Soft Reset command to the sensor.
    ///
    /// This performs a soft reset, it's unclear when this might be needed. It takes 20ms to
    /// complete and returns nothing.
    pub fn soft_reset(&mut self, delay: &mut impl DelayNs) -> Result<(), Error<I::Error>> {
        // SoftReset is 0b1011_1010. Equivalent to 0xBA, Section 5.3, page 8, Table 9.
        let command: [u8; 1] = [Command::SoftReset as u8];

        self.aht20
            .i2c
            .write(self.aht20.address, &command)
            .map_err(Error::I2c)?;
        // The datasheet in section 5.5 says there is a guarantee that the reset time does
        // not exceed 20ms. We wait the full 20ms to ensure you can trigger a measurement
        // immediately after this function.
        delay.delay_ms(20);

        Ok(())
    }

    /// Destroys this initialized driver and lets you release the I2C bus `I`
    pub fn destroy(self) -> AHT20<I> {
        self.aht20
    }
}

/// compute_crc uses the CRCu8 algoritm from crc-any. The parameter choice makes this a
/// "CRC-8-Dallas/Maxim".
///
/// The CRC invocation takes some parameters, which we get from the datasheet:
/// https://cdn-learn.adafruit.com/assets/assets/000/091/676/original/AHT20-datasheet-2020-4-16.pdf?1591047915
/// Section 5.4.4:
///
/// > CRC initial vaue is 0xFF, crc8 check polynomial CRC[7:0]=1+x**4 + x**5 + x**8
///
/// https://en.wikipedia.org/wiki/Cyclic_redundancy_check#Polynomial_representations_of_cyclic_redundancy_checks
/// You can find it in the table on wikipedia, under "CRC-8-Dallas/Maxim", 1-Wire bus.
///
/// This article explains how we get from `CRC[7:0]=1 + x**4 + x**5 + x**8` to `0x31` as the hex
/// representation: http://www.sunshine2k.de/articles/coding/crc/understanding_crc.html#ch72
///
/// The **N is the Nth bit (zero indexed).
/// > The most significant bit [(x**8)] is left out in the hexadecimal representation
///
/// That the leaves bit 0 (the +1 we do), 4, 5. So that gives us:
///
/// ```python
/// >>> hex(0x00110001)
/// '0x31'
/// ```
///
/// This is also what Knurling's test driver crate uses.
/// https://github.com/knurling-rs/test-driver-crate-example/blob/main/src/lib.rs#L59
/// which indicates this is either an I2C thing, or a common driver default as CRC parameters.
fn compute_crc(bytes: &[u8]) -> u8 {
    // Poly (0x31), bits (8), initial (0xff), final_xor (0x00), reflect (false).
    let mut crc = CRCu8::create_crc(0x31, 8, 0xff, 0x00, false);
    crc.digest(bytes);
    crc.get_crc()
}

#[cfg(test)]
mod tests {
    use super::AHT20Initialized;
    use super::Error;
    use super::AHT20;
    use super::SENSOR_ADDRESS;
    use embedded_hal_mock::eh1::delay::NoopDelay as MockDelay;
    use embedded_hal_mock::eh1::i2c::Mock as I2cMock;
    use embedded_hal_mock::eh1::i2c::Transaction;

    /// Test SensorStatus reporting being ready.
    #[test]
    fn sensorstatus_is_ready() {
        let status = super::SensorStatus::new(0x00);
        assert!(status.is_ready());
    }

    /// Test SensorStatus reporting being busy.
    #[test]
    fn sensorstatus_is_not_ready() {
        // 8th bit being 1 signifies "busy"
        // Equiv to 0x01 << 7, or 128 (dec) or 0x80 (hex)
        let status = super::SensorStatus::new(0b1000_0000);
        assert!(!status.is_ready());
    }

    /// Test SensorStatus reporting being calibrated.
    #[test]
    fn sensorstatus_is_calibrated() {
        // 4th bit being 1 signifies the sensor being calibrated.
        // Equiv to 0x01 << 3, or 8 (dec) or 0x08
        let status = super::SensorStatus::new(0b0000_1000);
        assert!(status.is_calibrated());
    }

    /// Test SensorStatus reporting being uncalibrated.
    #[test]
    fn sensorstatus_is_not_calibrated() {
        let status = super::SensorStatus::new(0b0000_0000);
        assert!(!status.is_calibrated());
    }

    /// Test creating new AHT20 sensors.
    ///
    /// Test that we can create multiple AHT20 devices. We test this because it's one of the
    /// measures of success for this driver.
    #[test]
    fn aht20_new() {
        // In the real app we'd used shared-bus to share the i2c bus between the two drivers, but
        // I think this is fine for a test.
        let mock_i2c_1 = I2cMock::new(&[]);
        let mock_i2c_2 = I2cMock::new(&[]);

        let aht20_1 = AHT20::new(mock_i2c_1, SENSOR_ADDRESS);
        let aht20_2 = AHT20::new(mock_i2c_2, SENSOR_ADDRESS);

        aht20_1.destroy().done();
        aht20_2.destroy().done();
    }

    /// Test reading a status byte.
    #[test]
    fn check_status() {
        let expectations = vec![Transaction::read(SENSOR_ADDRESS, vec![0b0000_1000])];
        let mock_i2c = I2cMock::new(&expectations);

        let mut aht20 = AHT20::new(mock_i2c, SENSOR_ADDRESS);
        let status = aht20.check_status().unwrap();
        assert!(status.is_calibrated());

        let mut mock = aht20.destroy();
        mock.done(); // verify expectations
    }

    /// Test sending the i2c Initialize command.
    #[test]
    fn send_initialize() {
        let expectations = vec![Transaction::write(
            SENSOR_ADDRESS,
            vec![
                super::Command::Initialize as u8,
                0b0000_1000, // 0x08
                0b0000_0000, // 0x00
            ],
        )];
        let mock_i2c = I2cMock::new(&expectations);

        let mut aht20 = AHT20::new(mock_i2c, SENSOR_ADDRESS);
        aht20.send_initialize().unwrap();

        let mut mock = aht20.destroy();
        mock.done(); // verify expectations
    }

    /// Initialize sensor, with the sensor reporting calibrated immediately.
    ///
    /// No call to send_initialize will be required.
    #[test]
    fn init_with_calibrated_sensor() {
        // This test has check_status return an already calibrated sensor. This means
        // that send_initialize is not called.
        let expectations = vec![
            // 4th bit being 1 signifies the sensor being calibrated.
            // Equiv to 0x01 << 3, or 8 (dec) or 0x08
            Transaction::read(SENSOR_ADDRESS, vec![0b0000_1000]),
        ];
        let mock_i2c = I2cMock::new(&expectations);
        let mut mock_delay = MockDelay::new();

        let aht20 = AHT20::new(mock_i2c, SENSOR_ADDRESS);
        let aht20_init = aht20.init(&mut mock_delay).unwrap();

        let mut mock = aht20_init.destroy().destroy();
        mock.done(); // verify expectations
    }

    /// Initialize sensor, with a report of an uncalibrated sensor.
    ///
    /// The sensor will report being uncalibrated once, then after initialization the sensor will
    /// report being calibrated.
    #[test]
    fn init_with_uncalibrated_sensor() {
        // This test has check_status return an uncalibrated sensor. With that, a call
        // to send_initialize is done to initialize and calibrate the sensor. A second
        // call to check_status verifies the new calibrated status.
        let expectations = vec![
            // 4th bit being 0 signifies the sensor not being calibrated.
            Transaction::read(SENSOR_ADDRESS, vec![0b0000_0000]),
            // This is send_initialize
            Transaction::write(
                SENSOR_ADDRESS,
                vec![
                    super::Command::Initialize as u8,
                    0b0000_1000, // 0x08
                    0b0000_0000, // 0x00
                ],
            ),
            // One more check_status will be called, this time with the 4th bit set
            // to 1 - signifying the sensor is now calibrated and we can finish the init.
            Transaction::read(SENSOR_ADDRESS, vec![0b0000_1000]),
        ];
        let mock_i2c = I2cMock::new(&expectations);
        let mut mock_delay = MockDelay::new();

        let aht20 = AHT20::new(mock_i2c, SENSOR_ADDRESS);
        let aht20_init = aht20.init(&mut mock_delay).unwrap();

        let mut mock = aht20_init.destroy().destroy();
        mock.done(); // verify expectations
    }

    /// Test sending the i2c SoftReset command.
    #[test]
    fn soft_reset() {
        let expectations = vec![Transaction::write(
            SENSOR_ADDRESS,
            vec![super::Command::SoftReset as u8],
        )];
        let mock_i2c = I2cMock::new(&expectations);
        let mut mock_delay = MockDelay::new();

        let aht20 = AHT20::new(mock_i2c, SENSOR_ADDRESS);
        let mut aht20_init = AHT20Initialized { aht20 };
        aht20_init.soft_reset(&mut mock_delay).unwrap();

        let mut mock = aht20_init.destroy().destroy();
        mock.done(); // verify expectations
    }

    /// Test sending the i2c TriggerMeasurement command.
    #[test]
    fn send_trigger_measurement() {
        let expectations = vec![Transaction::write(
            SENSOR_ADDRESS,
            vec![
                super::Command::TriggerMeasurement as u8,
                0b0011_0011, // 0x33
                0b0000_0000, // 0x00
            ],
        )];
        let mock_i2c = I2cMock::new(&expectations);

        let aht20 = AHT20::new(mock_i2c, SENSOR_ADDRESS);
        let mut aht20_init = AHT20Initialized { aht20 };
        aht20_init.send_trigger_measurement().unwrap();

        let mut mock = aht20_init.destroy().destroy();
        mock.done(); // verify expectations
    }

    /// Measure once, sensor reports ready at once.
    ///
    /// No wait is needed in this scenario.
    #[test]
    fn measure_once_immediately_ready() {
        let expectations = vec![
            // send_trigger_measurement
            Transaction::write(
                SENSOR_ADDRESS,
                vec![
                    super::Command::TriggerMeasurement as u8,
                    0b0011_0011, // 0x33
                    0b0000_0000, // 0x00
                ],
            ),
            // check_status called. 4th bit set to to 1 - signifying the sensor is calibrated 8th
            // bit set to 0 (not busy), signalling that a measurement is ready for us to read.
            Transaction::read(SENSOR_ADDRESS, vec![0b0000_1000]),
            // We can now read 7 bytes. status byte, 5 data bytes, crc byte.
            // These are taken from a run of the sensor.
            Transaction::read(
                SENSOR_ADDRESS,
                vec![
                    0b0001_1100, //  28, 0x1c - ready, calibrated, and some mystery flags.
                    //             bit 8 set to 0 is ready. bit 4 set is calibrated. bit 5
                    //             and 3 are described as 'reserved'.
                    0b0110_0101, // 101, 0x65 - first byte of humidity value
                    0b1011_0100, // 180, 0xb4 - second byte of humidity vaue
                    0b0010_0101, //  37, 0x25 - split byte. 4 bits humidity, 4 bits temperature.
                    0b1100_1101, // 205, 0xcd - first full byte of temperature.
                    0b0010_0110, //  38, 0x26 - second full byte of temperature.
                    0b1100_0110, // 198, 0xc6 - CRC
                ],
            ),
        ];
        let mock_i2c = I2cMock::new(&expectations);
        let mut mock_delay = MockDelay::new();

        let aht20 = AHT20::new(mock_i2c, SENSOR_ADDRESS);
        let mut aht20_init = AHT20Initialized { aht20 };
        aht20_init.measure_once(&mut mock_delay).unwrap();

        let mut mock = aht20_init.destroy().destroy();
        mock.done(); // verify expectations
    }

    /// Measure once, sensor erroniously reports ready at once, then correctly reports
    /// busy in the CRC-checked status byte causing an error.
    ///
    /// No wait is needed in this scenario.
    #[test]
    fn measure_once_ready_misreported() {
        let expectations = vec![
            // send_trigger_measurement
            Transaction::write(
                SENSOR_ADDRESS,
                vec![
                    super::Command::TriggerMeasurement as u8,
                    0b0011_0011, // 0x33
                    0b0000_0000, // 0x00
                ],
            ),
            // check_status called. 4th bit set to to 1 - signifying the sensor is calibrated 8th
            // bit set to 0 (not busy), signalling that a measurement is ready for us to read.
            // NOTE: This read says we're not busy, that is "ready".
            Transaction::read(SENSOR_ADDRESS, vec![0b0000_1000]),
            // We can now read 7 bytes. status byte, 5 data bytes, crc byte.
            // These are taken from a run of the sensor.
            Transaction::read(
                SENSOR_ADDRESS,
                vec![
                    0b1001_1100, // 156, 0x9c - busy, calibrated, and some mystery flags.
                    //             bit 8 set to 1 is busy. bit 4 set is calibrated. bit 5
                    //             and 3 are described as 'reserved'.
                    //             NOTE: this says busy, contradicting the ready above.
                    0b0110_0101, // 101, 0x65 - first byte of humidity value
                    0b1011_0100, // 180, 0xb4 - second byte of humidity vaue
                    0b0010_0101, //  37, 0x25 - split byte. 4 bits humidity, 4 bits temperature.
                    0b1100_1101, // 205, 0xcd - first full byte of temperature.
                    0b0010_0110, //  38, 0x26 - second full byte of temperature.
                    0b0010_1010, // 424, 0x2a - CRC
                ],
            ),
        ];
        let mock_i2c = I2cMock::new(&expectations);
        let mut mock_delay = MockDelay::new();

        let aht20 = AHT20::new(mock_i2c, SENSOR_ADDRESS);
        let mut aht20_init = AHT20Initialized { aht20 };
        // We received a ready from the check_status method, then a busy in the CRC-checked
        // status byte - and therefore we got the UnexpectedBusy.
        assert_eq!(
            aht20_init.measure_once(&mut mock_delay),
            Err(Error::UnexpectedBusy)
        );

        let mut mock = aht20_init.destroy().destroy();
        mock.done(); // verify expectations
    }

    /// Measure once, with a wait inserted.
    ///
    /// We signal via check_status that a wait should be inserted before another attempt to read
    /// data from the sensor is made.
    #[test]
    fn measure_once_wait_once() {
        let expectations = vec![
            // send_trigger_measurement
            Transaction::write(
                SENSOR_ADDRESS,
                vec![
                    super::Command::TriggerMeasurement as u8,
                    0b0011_0011, // 0x33
                    0b0000_0000, // 0x00
                ],
            ),
            // check_status called. 4th bit set to to 1 - signifying the sensor is calibrated 8th
            // bit set to 1 (busy), signalling that we should wait for the sensor.
            Transaction::read(SENSOR_ADDRESS, vec![0b1000_1000]),
            // Next time round, we say that the sensor is good to go.
            Transaction::read(SENSOR_ADDRESS, vec![0b0000_1000]),
            // We can now read 7 bytes. status byte, 5 data bytes, crc byte.
            // These are taken from a run of the sensor.
            Transaction::read(
                SENSOR_ADDRESS,
                vec![
                    0b0001_1100, //  28, 0x1c - ready, calibrated, and some mystery flags.
                    //             bit 8 set to 0 is ready. bit 4 set is calibrated. bit 5
                    //             and 3 are described as 'reserved'.
                    0b0110_0101, // 101, 0x65 - first byte of humidity value
                    0b1011_0100, // 180, 0xb4 - second byte of humidity vaue
                    0b0010_0101, //  37, 0x25 - split byte. 4 bits humidity, 4 bits temperature.
                    0b1100_1101, // 205, 0xcd - first full byte of temperature.
                    0b0010_0110, //  38, 0x26 - second full byte of temperature.
                    0b1100_0110, // 198, 0xc6 - CRC
                ],
            ),
        ];
        let mock_i2c = I2cMock::new(&expectations);
        let mut mock_delay = MockDelay::new();

        let aht20 = AHT20::new(mock_i2c, SENSOR_ADDRESS);
        let mut aht20_init = AHT20Initialized { aht20 };
        aht20_init.measure_once(&mut mock_delay).unwrap();

        let mut mock = aht20_init.destroy().destroy();
        mock.done(); // verify expectations
    }

    /// Single measurement pass with bad CRC.
    ///
    /// Intentionally corrupt the read data to make sure we get a CRC error.
    #[test]
    fn measure_once_bad_crc() {
        let expectations = vec![
            // send_trigger_measurement
            Transaction::write(
                SENSOR_ADDRESS,
                vec![
                    super::Command::TriggerMeasurement as u8,
                    0b0011_0011, // 0x33
                    0b0000_0000, // 0x00
                ],
            ),
            // Check status, and  we say that the sensor is good to go.
            Transaction::read(SENSOR_ADDRESS, vec![0b0000_1000]),
            // We can now read 7 bytes. status byte, 5 data bytes, crc byte.
            // These are taken from a run of the sensor.
            Transaction::read(
                SENSOR_ADDRESS,
                vec![
                    0b0001_1100, //  28, 0x1c - ready, calibrated, and some mystery flags.
                    //             bit 8 set to 0 is ready. bit 4 set is calibrated. bit 5
                    //             and 3 are described as 'reserved'.
                    0b0110_0101, // 101, 0x65 - first byte of humidity value
                    0b1011_0100, // 180, 0xb4 - second byte of humidity vaue
                    0b0010_0101, //  37, 0x25 - split byte. 4 bits humidity, 4 bits temperature.
                    0b1100_1101, // 205, 0xcd - first full byte of temperature.
                    0b0010_0111, //  39, 0x27 - second full byte of temperature.
                    //  NOTE: This should be 38, 0x26, but is intentionally corrupted
                    //        so that the CRC won't match. Last bit flipped from 0 to 1.
                    0b1100_0110, // 198, 0xc6 - CRC
                ],
            ),
        ];
        let mock_i2c = I2cMock::new(&expectations);
        let mut mock_delay = MockDelay::new();

        // test and verify
        let aht20 = AHT20::new(mock_i2c, SENSOR_ADDRESS);
        let mut aht20_init = AHT20Initialized { aht20 };
        match aht20_init.measure_once(&mut mock_delay) {
            Ok(_) => panic!("CRC is wrong and measure_once should not pass."),
            Err(err_type) => assert_eq!(err_type, Error::InvalidCrc),
        }

        let mut mock = aht20_init.destroy().destroy();
        mock.done(); // verify expectations
    }

    /// Test a measurement.
    ///
    /// This uses data from an actual sensor run.
    #[test]
    fn measure() {
        // setup
        let expectations = vec![
            // send_trigger_measurement
            Transaction::write(
                SENSOR_ADDRESS,
                vec![
                    super::Command::TriggerMeasurement as u8,
                    0b0011_0011, // 0x33
                    0b0000_0000, // 0x00
                ],
            ),
            // check_status - with ready bit set to 'ready' (off)
            Transaction::read(SENSOR_ADDRESS, vec![0b0000_1000]),
            // We can now read 7 bytes. status byte, 5 data bytes, crc byte.
            // These are taken from a run of the sensor.
            Transaction::read(
                SENSOR_ADDRESS,
                vec![
                    0b0001_1100, //  28, 0x1c - ready, calibrated, and some mystery flags.
                    //             bit 8 set to 0 is ready. bit 4 set is calibrated. bit 5
                    //             and 3 are described as 'reserved'.
                    0b0110_0101, // 101, 0x65 - first byte of humidity value
                    0b1011_0100, // 180, 0xb4 - second byte of humidity vaue
                    0b0010_0101, //  37, 0x25 - split byte. 4 bits humidity, 4 bits temperature.
                    0b1100_1101, // 205, 0xcd - first full byte of temperature.
                    0b0010_0110, //  38, 0x26 - second full byte of temperature.
                    0b1100_0110, // 198, 0xc6 - CRC
                ],
            ),
        ];
        let mock_i2c = I2cMock::new(&expectations);
        let mut mock_delay = MockDelay::new();

        // test
        let aht20 = AHT20::new(mock_i2c, SENSOR_ADDRESS);
        let mut aht20_init = AHT20Initialized { aht20 };
        let measurement = aht20_init.measure(&mut mock_delay).unwrap();

        // verification
        let mut mock = aht20_init.destroy().destroy();
        mock.done(); // verify expectations

        // Temp was 22.52C and humidity 39.73% when above data taken.
        assert!(measurement.temperature > 22.5);
        assert!(measurement.temperature < 22.6);
        assert!(measurement.humidity > 39.7 && measurement.humidity < 39.8);
    }

    /// Test a measurement, without using floating point math.
    ///
    /// This uses data from an actual sensor run.
    #[test]
    fn measure_no_fp() {
        // setup
        let expectations = vec![
            // send_trigger_measurement
            Transaction::write(
                SENSOR_ADDRESS,
                vec![
                    super::Command::TriggerMeasurement as u8,
                    0b0011_0011, // 0x33
                    0b0000_0000, // 0x00
                ],
            ),
            // check_status - with ready bit set to 'ready' (off)
            Transaction::read(SENSOR_ADDRESS, vec![0b0000_1000]),
            // We can now read 7 bytes. status byte, 5 data bytes, crc byte.
            // These are taken from a run of the sensor.
            Transaction::read(
                SENSOR_ADDRESS,
                vec![
                    0b0001_1100, //  28, 0x1c - ready, calibrated, and some mystery flags.
                    //             bit 8 set to 0 is ready. bit 4 set is calibrated. bit 5
                    //             and 3 are described as 'reserved'.
                    0b0110_0101, // 101, 0x65 - first byte of humidity value
                    0b1011_0100, // 180, 0xb4 - second byte of humidity vaue
                    0b0010_0101, //  37, 0x25 - split byte. 4 bits humidity, 4 bits temperature.
                    0b1100_1101, // 205, 0xcd - first full byte of temperature.
                    0b0010_0110, //  38, 0x26 - second full byte of temperature.
                    0b1100_0110, // 198, 0xc6 - CRC
                ],
            ),
        ];
        let mock_i2c = I2cMock::new(&expectations);
        let mut mock_delay = MockDelay::new();

        // test
        let aht20 = AHT20::new(mock_i2c, SENSOR_ADDRESS);
        let mut aht20_init = AHT20Initialized { aht20 };
        let measurement = aht20_init.measure_no_fp(&mut mock_delay).unwrap();

        // verification
        let mut mock = aht20_init.destroy().destroy();
        mock.done(); // verify expectations

        // Temp was 22.52C and humidity 39.73% when above data taken.
        // No fp mode will found that to 22.0C and 39.0%.
        println!("temp: {:.2}", measurement.temperature);
        println!("humidity: {:.2}", measurement.humidity);
        assert!(measurement.temperature == 22.0);
        assert!(measurement.humidity == 39.0);
    }

    /// Test a valid CRC invocation.
    /// Test a valid CRC invocation.
    #[test]
    fn crc_correct() {
        // Example from the Interface Specification document.
        assert_eq!(super::compute_crc(&[0xBE, 0xEF]), 0x92);
    }

    /// Test a CRC call that does not match.
    #[test]
    fn crc_wrong() {
        // Changed example from the Interface Specification document. This should not match - the
        // bytes going in are changed from the known good values, but the expected result is the
        // same.
        assert_ne!(super::compute_crc(&[0xFF, 0xFF]), 0x92);
    }
}
