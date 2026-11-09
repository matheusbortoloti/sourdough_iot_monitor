use embedded_hal::{delay::DelayNs, i2c::I2c};

use ens160_aq::Ens160;
use esp_hal::{
    delay::Delay,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub enum Ens160InitError<E> {
    InitializationFailed,
    Driver(ens160_aq::error::Error<E>),
}

#[derive(Debug)]
pub enum Ens160MeasurementError<E> {
    Timeout,
    Driver(ens160_aq::error::Error<E>),
}

pub fn new<I2C, D>(i2c: I2C, delay: D) -> Ens160<I2C, D>
where
    I2C: I2c,
    D: DelayNs,
{
    Ens160::new_secondary_address(i2c, delay) // Necessary when ENS160 is configured to use I2C address 0x53
}

pub fn initialize<I2C, D>(sensor: &mut Ens160<I2C, D>) -> Result<(), Ens160InitError<I2C::Error>>
where
    I2C: I2c,
    D: DelayNs,
{
    match sensor.initialize() {
        Ok(true) => Ok(()),
        Ok(false) => Err(Ens160InitError::InitializationFailed),
        Err(error) => Err(Ens160InitError::Driver(error)),
    }
}

pub fn get_measurements<I2C, D>(
    sensor: &mut Ens160<I2C, D>,
    timeout_ms: u64,
    temperature: f32,
    humidity: f32,
) -> Result<ens160_aq::data::Measurements, Ens160MeasurementError<I2C::Error>>
where
    I2C: I2c,
    D: DelayNs,
{
    // Apply environmental compensation before reading measurements.
    sensor
        .set_temp_rh_comp(temperature, libm::roundf(humidity) as u16)
        .map_err(Ens160MeasurementError::Driver)?;

    let start = Instant::now();
    let mut delay = Delay::new();

    loop {
        let status = sensor
            .get_status()
            .map_err(Ens160MeasurementError::Driver)?;

        if status.new_data_ready() {
            return sensor
                .get_measurements()
                .map_err(Ens160MeasurementError::Driver);
        }

        if start.elapsed() >= Duration::from_millis(timeout_ms) {
            return Err(Ens160MeasurementError::Timeout);
        }

        delay.delay_ms(50);
    }
}
