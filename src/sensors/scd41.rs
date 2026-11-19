use embedded_hal::i2c::I2c;
use esp_hal::delay::Delay;
use scd4x::Scd4x;

pub struct Scd41Measurements {
    pub co2_ppm: u16,
    pub temperature_celsius: f32,
    pub humidity_percent: f32,
}

pub type Scd41Driver<I2C> = Scd4x<I2C, Delay>;

pub fn initialize<I2C>(i2c: I2C) -> Result<Scd41Driver<I2C>, scd4x::Error<I2C::Error>>
where
    I2C: I2c,
{
    let mut sensor = Scd4x::new(i2c, Delay::new());

    sensor.start_periodic_measurement()?;

    Ok(sensor)
}

pub fn get_measurements<I2C>(
    sensor: &mut Scd41Driver<I2C>,
) -> Result<Option<Scd41Measurements>, scd4x::Error<I2C::Error>>
where
    I2C: I2c,
{
    // Check whether a new measurement is available.
    if !sensor.data_ready_status()? {
        return Ok(None);
    }

    // Read and convert the measurement.
    let measurement = sensor.measurement()?;

    Ok(Some(Scd41Measurements {
        co2_ppm: measurement.co2,
        temperature_celsius: measurement.temperature,
        humidity_percent: measurement.humidity,
    }))
}
