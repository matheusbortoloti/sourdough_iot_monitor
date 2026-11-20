use embedded_hal::i2c::I2c;
use esp_hal::delay::Delay;
use scd4x::Scd4x;

pub struct Scd41Measurements {
    pub co2_ppm: u16,
    pub temperature_celsius: f32,
    pub humidity_percent: f32,
}

#[derive(Debug)]
pub enum Scd41MeasurementError<E> {
    Sensor(scd4x::Error<E>),
    Timeout,
}

pub type Scd41Driver<I2C> = Scd4x<I2C, Delay>;

pub fn initialize<I2C>(i2c: I2C) -> Result<Scd41Driver<I2C>, scd4x::Error<I2C::Error>>
where
    I2C: I2c,
{
    let mut sensor = Scd4x::new(i2c, Delay::new());

    // Stop any measurement that might have been running before the ESP32 was reset
    sensor.stop_periodic_measurement()?;
    sensor.start_periodic_measurement()?;

    Ok(sensor)
}

pub fn get_measurements<I2C>(
    sensor: &mut Scd41Driver<I2C>,
) -> Result<Scd41Measurements, Scd41MeasurementError<I2C::Error>>
where
    I2C: I2c,
{
    const POLL_INTERVAL_MS: u32 = 400;
    const MEASUREMENT_TIMEOUT_MS: u32 = 6_000;

    let delay = Delay::new();
    let mut elapsed_ms = 0;

    loop {
        if sensor
            .data_ready_status()
            .map_err(Scd41MeasurementError::Sensor)?
        {
            let measurement = sensor
                .measurement()
                .map_err(Scd41MeasurementError::Sensor)?;

            return Ok(Scd41Measurements {
                co2_ppm: measurement.co2,
                temperature_celsius: measurement.temperature,
                humidity_percent: measurement.humidity,
            });
        }

        if elapsed_ms >= MEASUREMENT_TIMEOUT_MS {
            return Err(Scd41MeasurementError::Timeout);
        }

        delay.delay_millis(POLL_INTERVAL_MS);
        elapsed_ms += POLL_INTERVAL_MS;
    }
}
