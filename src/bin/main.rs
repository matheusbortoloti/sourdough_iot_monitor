#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use esp_hal::delay::Delay;
use esp_hal::clock::CpuClock;
use esp_hal::main;
use esp_println::println;
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
use embedded_hal::delay::DelayNs;

use core::cell::RefCell;

use sourdough_monitor::sensors::{self};

const SAMPLE_INTERVAL_MS: u32 = 1000;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

fn halt() -> ! {
    println!("FATAL: Sensor initialization failed. System halted.");
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
    

    let mut delay = Delay::new();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let i2c = I2c::new(peripherals.I2C0, I2cConfig::default())
        .unwrap()
        .with_sda(peripherals.GPIO8)
        .with_scl(peripherals.GPIO9);

    let i2c_bus = RefCell::new(i2c);

    let mut sensors = match sensors::initialize(&i2c_bus) {
        Ok(sensors) => {
            println!("All sensors initialized successfully!");
            sensors
        }
        Err(error) => {
            println!("FATAL: Sensor initialization failed: {:?}", error);
            halt();
        }
    };

    loop {
        match sensors::get_measurements(&mut sensors, &mut delay) {
            Ok(readings) => {
                println!("Distance: {} mm", readings.distance_mm);
                println!("ENS160: {:?}", readings.ens160);
                println!("AHT21: {:?}", readings.aht21);
            }
            Err(error) => {
                println!("Measurement error: {:?}", error);
            }
        }

        delay.delay_ms(SAMPLE_INTERVAL_MS);
    }
}
