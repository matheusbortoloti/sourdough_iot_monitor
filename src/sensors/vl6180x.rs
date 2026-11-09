use embedded_hal::i2c::I2c;
use embedded_hal::delay::DelayNs;

const VL6180X_ADDRESS: u8 = 0x29;

const MODEL_ID_REGISTER: u16 = 0x0000;
const EXPECTED_MODEL_ID: u8 = 0xB4;
const FRESH_OUT_OF_RESET_REGISTER: u16 = 0x0016;

const RANGE_START_REGISTER: u16 = 0x0018;
const RANGE_STATUS_REGISTER: u16 = 0x004D;
const INTERRUPT_STATUS_REGISTER: u16 = 0x004F;
const RANGE_VALUE_REGISTER: u16 = 0x0062;
const INTERRUPT_CLEAR_REGISTER: u16 = 0x0015;

const RANGE_READY_MASK: u8 = 0x07;
const RANGE_READY_VALUE: u8 = 0x04;

const SYSTEM_INTERRUPT_CONFIG_GPIO: u16 = 0x0014;
const SYSRANGE_MAX_CONVERGENCE_TIME: u16 = 0x001C;

const MOVING_AVERAGE_WINDOW: usize = 10;

#[derive(Debug)]
pub enum Vl6180xInitError<E> {
    Communication(E),
    UnexpectedModelId(u8),
}

#[derive(Debug)]
pub enum Vl6180xMeasurementError<E> {
    Communication(E),
    Timeout,
    RangeError(u8),
    ReferenceNotEstablished,
}


pub struct Vl6180x<I2C> {
    i2c: I2C,
    reference_distance_mm: Option<u8>,
    moving_average: MovingAverage,
}

impl<I2C> Vl6180x<I2C> {
    pub fn new(i2c: I2C) -> Self {
    Self {
        i2c,
        reference_distance_mm: None,
        moving_average: MovingAverage::new(),
    }
}
}


struct MovingAverage {
    samples: [i16; MOVING_AVERAGE_WINDOW],
    count: usize,
    next_index: usize,
    sum: i32,
}

impl MovingAverage {
    fn new() -> Self {
        Self {
            samples: [0; MOVING_AVERAGE_WINDOW],
            count: 0,
            next_index: 0,
            sum: 0,
        }
    }

    fn update(&mut self, new_sample: i16) -> i16 {
        // Remove the oldest sample when the window is full.
        if self.count == MOVING_AVERAGE_WINDOW {
            self.sum -= self.samples[self.next_index] as i32;
        } else {
            self.count += 1;
        }

        // Store the new sample.
        self.samples[self.next_index] = new_sample;
        self.sum += new_sample as i32;

        // Advance the circular buffer index.
        self.next_index =
            (self.next_index + 1) % MOVING_AVERAGE_WINDOW;

        // Calculate the average.
        (self.sum / self.count as i32) as i16
    }

    fn reset(&mut self) {
        self.samples = [0; MOVING_AVERAGE_WINDOW];
        self.count = 0;
        self.next_index = 0;
        self.sum = 0;
    }
}


fn read_register<I2C>(
    i2c: &mut I2C,
    register: u16,
) -> Result<u8, I2C::Error>
where
    I2C: I2c,
{
    let register_address = register.to_be_bytes();
    let mut value = [0u8; 1];

    i2c.write_read(
        VL6180X_ADDRESS,
        &register_address,
        &mut value,
    )?;

    Ok(value[0])
}

fn write_register<I2C>(
    i2c: &mut I2C,
    register: u16,
    value: u8,
) -> Result<(), I2C::Error>
where
    I2C: I2c,
{
    let register_address = register.to_be_bytes();

    i2c.write(
        VL6180X_ADDRESS,
        &[
            register_address[0],
            register_address[1],
            value,
        ],
    )
}


pub fn initialize_sensor<I2C>(
    i2c: &mut I2C,
) -> Result<(), Vl6180xInitError<I2C::Error>>
where
    I2C: I2c,
{
    let model_id = read_register(i2c, MODEL_ID_REGISTER)
        .map_err(Vl6180xInitError::Communication)?;

    if model_id != EXPECTED_MODEL_ID {
        return Err(
            Vl6180xInitError::UnexpectedModelId(model_id)
        );
    }

    let fresh_out_of_reset =
        read_register(i2c, FRESH_OUT_OF_RESET_REGISTER)
            .map_err(Vl6180xInitError::Communication)?;

    if fresh_out_of_reset == 1 {
        const INITIALIZATION_REGISTERS: &[(u16, u8)] = &[
            (0x0207, 0x01),
            (0x0208, 0x01),
            (0x0096, 0x00),
            (0x0097, 0xFD),
            (0x00E3, 0x01),
            (0x00E4, 0x03),
            (0x00E5, 0x02),
            (0x00E6, 0x01),
            (0x00E7, 0x03),
            (0x00F5, 0x02),
            (0x00D9, 0x05),
            (0x00DB, 0xCE),
            (0x00DC, 0x03),
            (0x00DD, 0xF8),
            (0x009F, 0x00),
            (0x00A3, 0x3C),
            (0x00B7, 0x00),
            (0x00BB, 0x3C),
            (0x00B2, 0x09),
            (0x00CA, 0x09),
            (0x0198, 0x01),
            (0x01B0, 0x17),
            (0x01AD, 0x00),
            (0x00FF, 0x05),
            (0x0100, 0x05),
            (0x0199, 0x05),
            (0x01A6, 0x1B),
            (0x01AC, 0x3E),
            (0x01A7, 0x1F),
            (0x0030, 0x00),
        ];

        for &(register, value) in INITIALIZATION_REGISTERS {
            write_register(i2c, register, value)
                .map_err(Vl6180xInitError::Communication)?;
        }

        write_register(i2c, FRESH_OUT_OF_RESET_REGISTER, 0)
            .map_err(Vl6180xInitError::Communication)?;
    }

    // Configure range-ready interrupt.
    write_register(i2c, SYSTEM_INTERRUPT_CONFIG_GPIO, 0x24)
        .map_err(Vl6180xInitError::Communication)?;

    // Set maximum range convergence time to 49 ms.
    write_register(i2c, SYSRANGE_MAX_CONVERGENCE_TIME, 0x31)
        .map_err(Vl6180xInitError::Communication)?;

    Ok(())
}

pub fn read_distance_mm<I2C, D>(
    i2c: &mut I2C,
    delay: &mut D,
    timeout_ms: u32,
) -> Result<u8, Vl6180xMeasurementError<I2C::Error>>
where
    I2C: I2c,
    D: DelayNs,
{
    // Wait until the sensor is ready for a new measurement.
    let mut elapsed_ms = 0;

    loop {
        let status = read_register(i2c, RANGE_STATUS_REGISTER)
            .map_err(Vl6180xMeasurementError::Communication)?;

        if status & 0x01 != 0 {
            break;
        }

        if elapsed_ms >= timeout_ms {
            return Err(Vl6180xMeasurementError::Timeout);
        }

        delay.delay_ms(1);
        elapsed_ms += 1;
    }

    // Clear any previous interrupt conditions.
    write_register(i2c, INTERRUPT_CLEAR_REGISTER, 0x07)
        .map_err(Vl6180xMeasurementError::Communication)?;

    // Start a single-shot measurement.
    write_register(i2c, RANGE_START_REGISTER, 0x01)
        .map_err(Vl6180xMeasurementError::Communication)?;

    // Wait for measurement completion.
    elapsed_ms = 0;

    loop {
        let status = match read_register(i2c, INTERRUPT_STATUS_REGISTER) {
            Ok(status) => status,
            Err(error) => {
                let _ = write_register(i2c, INTERRUPT_CLEAR_REGISTER, 0x07);
                return Err(Vl6180xMeasurementError::Communication(error));
            }
        };

        if status & RANGE_READY_MASK == RANGE_READY_VALUE {
            break;
        }

        if elapsed_ms >= timeout_ms {
            let _ = write_register(i2c, INTERRUPT_CLEAR_REGISTER, 0x07);
            return Err(Vl6180xMeasurementError::Timeout);
        }

        delay.delay_ms(1);
        elapsed_ms += 1;
    }

    // Read the range status.
    let range_status = match read_register(i2c, RANGE_STATUS_REGISTER) {
        Ok(status) => status,
        Err(error) => {
            let _ = write_register(i2c, INTERRUPT_CLEAR_REGISTER, 0x07);
            return Err(Vl6180xMeasurementError::Communication(error));
        }
    };

    // Read the distance.
    let distance = match read_register(i2c, RANGE_VALUE_REGISTER) {
        Ok(distance) => distance,
        Err(error) => {
            let _ = write_register(i2c, INTERRUPT_CLEAR_REGISTER, 0x07);
            return Err(Vl6180xMeasurementError::Communication(error));
        }
    };

    // Always clear the interrupt after a completed measurement.
    write_register(i2c, INTERRUPT_CLEAR_REGISTER, 0x07)
        .map_err(Vl6180xMeasurementError::Communication)?;

    // Check the measurement result.
    let range_error = range_status >> 4;

    if range_error != 0 {
        return Err(Vl6180xMeasurementError::RangeError(range_error));
    }

    Ok(distance)
}

impl<I2C> Vl6180x<I2C>
where
    I2C: I2c,
{
    pub fn initialize(&mut self) -> Result<(), Vl6180xInitError<I2C::Error>> {
        initialize_sensor(&mut self.i2c)
    }

    pub fn establish_reference<D>(
        &mut self,
        delay: &mut D,
        timeout_ms: u32,
    ) -> Result<u8, Vl6180xMeasurementError<I2C::Error>>
    where
        D: DelayNs,
    {
        let distance = read_distance_mm(
            &mut self.i2c,
            delay,
            timeout_ms,
        )?;

        self.moving_average.reset();
        self.reference_distance_mm = Some(distance);

        Ok(distance)
    }

    pub fn get_relative_height<D>(
        &mut self,
        delay: &mut D,
        timeout_ms: u32,
    ) -> Result<i16, Vl6180xMeasurementError<I2C::Error>>
    where
        D: DelayNs,
    {
        let reference = match self.reference_distance_mm {
            Some(distance) => distance,
            None => {
                return Err(
                    Vl6180xMeasurementError::ReferenceNotEstablished
                );
            }
        };

        let current_distance = read_distance_mm(
            &mut self.i2c,
            delay,
            timeout_ms,
        )?;

        let relative_height = reference as i16 - current_distance as i16;

        let filtered_height = self.moving_average.update(relative_height);

        Ok(filtered_height)
    }
}