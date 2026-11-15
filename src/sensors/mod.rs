use aht30::Aht20;
use core::cell::RefCell;
use embedded_hal::i2c::I2c;
use embedded_hal_bus::i2c::RefCellDevice;
use ens160_aq::Ens160;
use esp_hal::delay::Delay;

pub mod aht21;
pub mod ens160;
pub mod vl6180x;

type Vl6180xDriver<'a, I2C> = vl6180x::Vl6180x<RefCellDevice<'a, I2C>>;
type Ens160Driver<'a, I2C> = Ens160<RefCellDevice<'a, I2C>, Delay>;
type Aht21Driver<'a, I2C> = Aht20<RefCellDevice<'a, I2C>, Delay>;

#[derive(Debug)]
pub enum SensorInitError<E> {
    Vl6180x(vl6180x::Vl6180xInitError<E>),
    Ens160(ens160::Ens160InitError<E>),
    Aht21(aht21::Aht21InitError),
}

#[derive(Debug)]
pub enum SensorMeasurementError<E> {
    Vl6180x(vl6180x::Vl6180xMeasurementError<E>),
    Ens160(ens160::Ens160MeasurementError<E>),
    Aht21(aht21::Aht21MeasurementError),
}

pub struct SensorReadings {
    pub relative_height_mm: f32,
    pub ens160: ens160_aq::data::Measurements,
    pub aht21: aht21::Aht21Measurements,
}

pub struct InitializedSensors<Vl6180xDriver, Ens160Driver, Aht21Driver> {
    pub vl6180x: Vl6180xDriver,
    pub ens160: Ens160Driver,
    pub aht21: Aht21Driver,
}

pub fn initialize<'a, I2C>(
    i2c_bus: &'a RefCell<I2C>,
) -> Result<
    InitializedSensors<Vl6180xDriver<'a, I2C>, Ens160Driver<'a, I2C>, Aht21Driver<'a, I2C>>,
    SensorInitError<I2C::Error>,
>
where
    I2C: I2c,
{
    // 1. Initialize VL6180X
    let mut vl6180x_driver = vl6180x::Vl6180x::new(RefCellDevice::new(i2c_bus));

    vl6180x_driver
        .initialize()
        .map_err(SensorInitError::Vl6180x)?;

    // 2. Initialize ENS160
    let mut ens160 = ens160::new(RefCellDevice::new(i2c_bus), Delay::new());

    ens160::initialize(&mut ens160).map_err(SensorInitError::Ens160)?;

    // 3. Initialize AHT21
    let mut aht21 = aht21::new(RefCellDevice::new(i2c_bus), Delay::new());

    aht21::initialize(&mut aht21).map_err(SensorInitError::Aht21)?;

    // 4. Return initialized drivers
    Ok(InitializedSensors {
        vl6180x: vl6180x_driver,
        ens160,
        aht21,
    })
}

pub fn get_measurements<I2C>(
    sensors: &mut InitializedSensors<
        Vl6180xDriver<'_, I2C>,
        Ens160Driver<'_, I2C>,
        Aht21Driver<'_, I2C>,
    >,
    delay: &mut Delay,
) -> Result<SensorReadings, SensorMeasurementError<I2C::Error>>
where
    I2C: I2c,
{
    let aht21 =
        aht21::get_measurements(&mut sensors.aht21).map_err(SensorMeasurementError::Aht21)?;

    let ens160 =
        ens160::get_measurements(&mut sensors.ens160, 2000, aht21.temperature, aht21.humidity)
            .map_err(SensorMeasurementError::Ens160)?;

    let relative_height_mm = sensors
        .vl6180x
        .get_relative_height(delay, 1000)
        .map_err(SensorMeasurementError::Vl6180x)?;

    Ok(SensorReadings {
        relative_height_mm,
        ens160,
        aht21,
    })
}
