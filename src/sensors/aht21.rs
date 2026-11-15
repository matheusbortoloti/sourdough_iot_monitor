use aht30::{AHT20_DEFAULT_ADDR, Aht20};
use embedded_hal::{delay::DelayNs, i2c::I2c};

#[derive(Debug)]
pub struct Aht21Measurements {
    pub temperature: f32,
    pub temperature_calibrated: f32,
    pub humidity: f32,
}

#[derive(Debug)]
pub enum Aht21InitError {
    CalibrationFailed(aht30::SensorError),
}

#[derive(Debug)]
pub enum Aht21MeasurementError {
    ReadFailed(aht30::SensorError),
}

pub fn new<I2C, D>(i2c: I2C, delay: D) -> Aht20<I2C, D>
where
    I2C: I2c,
    D: DelayNs,
{
    Aht20::new(AHT20_DEFAULT_ADDR, i2c, delay)
}

pub fn initialize<I2C, D>(sensor: &mut Aht20<I2C, D>) -> Result<(), Aht21InitError>
where
    I2C: I2c,
    D: DelayNs,
{
    sensor
        .calibrate()
        .map_err(Aht21InitError::CalibrationFailed)
}

pub fn get_measurements<I2C, D>(
    sensor: &mut Aht20<I2C, D>,
) -> Result<Aht21Measurements, Aht21MeasurementError>
where
    I2C: I2c,
    D: DelayNs,
{
    const TEMPERATURE_OFFSET_CELSIUS: f32 = 15.0; // determined empirically 

    let measurement = sensor
        .read(true)
        .map_err(Aht21MeasurementError::ReadFailed)?;

    let (humidity, temperature) = measurement.decode();

    Ok(Aht21Measurements {
        temperature: temperature,
        temperature_calibrated: temperature - TEMPERATURE_OFFSET_CELSIUS,
        humidity,
    })
}
