#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use esp_hal::clock::CpuClock;
use esp_hal::main;
use esp_hal::time::{Duration, Instant};
use esp_println::println;
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
use ens160_aq::Ens160;
use esp_hal::delay::Delay;
use core::cell::RefCell;
use embedded_hal_bus::i2c::RefCellDevice;
use embedded_hal::i2c::I2c as _;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[main]
fn main() -> ! {
    // generator version: 2.0.0
    // generator parameters: -o esp32c3

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let i2c = I2c::new(peripherals.I2C0, I2cConfig::default())
    .unwrap()
    .with_sda(peripherals.GPIO8)
    .with_scl(peripherals.GPIO9);

    let i2c_bus = RefCell::new(i2c);

    let mut model_id = [0u8; 1];

    let mut vl6180x_i2c = RefCellDevice::new(&i2c_bus);

    match vl6180x_i2c.write_read(0x29, &[0x00, 0x00], &mut model_id) {
        Ok(()) => println!("VL6180X model ID: {:#04X}", model_id[0]),
        Err(error) => println!("VL6180X communication error: {:?}", error),
    }

    let mut ens160 = Ens160::new_secondary_address(
        RefCellDevice::new(&i2c_bus),
        Delay::new(),
    );

    match ens160.initialize() {
        Ok(_) => {
            println!("ENS160 initialized!");

            match ens160.get_part_id() {
                Ok(id) => println!("ENS160 part ID: {:#06X}", id),
                Err(error) => println!("ENS160 ID error: {:?}", error),
            }
        }
        Err(error) => {
            println!("ENS160 initialization error: {:?}", error);
        }
    }

    loop {
        match ens160.get_status() {
            Ok(status) => {
                if status.new_data_ready() {
                    match ens160.get_measurements() {
                        Ok(measurements) => {
                            println!("ENS160 measurements:");
                            println!("AQI: {:?}", measurements.air_quality_index);
                            println!("eCO2: {:?}", measurements.co2eq_ppm);
                            println!("TVOC: {:?}", measurements.tvoc_ppb);
                        }
                        Err(error) => {
                            println!("Measurement error: {:?}", error);
                        }
                    }
                } else {
                    println!("ENS160: waiting for measurements");
                }
            }
            Err(error) => {
                println!("ENS160 status error: {:?}", error);
            }
        }

        let delay_start = Instant::now();
        while delay_start.elapsed() < Duration::from_millis(1000) {}
    }


//    loop {
//         println!("Sourdough Monitor is alive!");

//         let delay_start = Instant::now();
//         while delay_start.elapsed() < Duration::from_millis(1000) {}
    }
    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.2.2/examples
// }
