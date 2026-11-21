#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

extern crate alloc;

use core::cell::RefCell;
use embassy_executor::Spawner;
use embassy_time::{Duration, Instant, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::i2c::master::{Config as I2cConfig, I2c};
use esp_hal::peripherals::{GPIO8, GPIO9, I2C0};
use esp_hal::ram;
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;
use mbedtls_rs::Tls;

use sourdough_monitor::cloud::thingspeak::ThingSpeak;
use sourdough_monitor::network::Network;
use sourdough_monitor::sensors::{self};

const SAMPLE_INTERVAL_MS: u64 = 20 * 1000; /* 20 seconds */
const UPLOAD_INTERVAL_MS: u64 = 5 * 60 * 1000; /* 5 minutes */

#[derive(Clone, Copy, Debug)]
struct LatestReadings {
    relative_height_mm: f32,
    temperature_celsius: f32,
    humidity_percent: f32,
    tvoc_ppb: u16,
    co2_ppm: u16,
}

struct SharedState {
    latest_readings: critical_section::Mutex<RefCell<Option<LatestReadings>>>,
}

impl SharedState {
    const fn new() -> Self {
        Self {
            latest_readings: critical_section::Mutex::new(RefCell::new(None)),
        }
    }

    fn update(&self, readings: LatestReadings) {
        critical_section::with(|cs| {
            *self.latest_readings.borrow(cs).borrow_mut() = Some(readings);
        });
    }

    fn latest_readings(&self) -> Option<LatestReadings> {
        critical_section::with(|cs| *self.latest_readings.borrow(cs).borrow())
    }
}

static SHARED_STATE: SharedState = SharedState::new();

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

fn halt() -> ! {
    println!("FATAL: Sensor initialization failed. System halted.");
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    // Initialize heap memory for Wi-Fi
    esp_alloc::heap_allocator!(#[ram(reclaimed)] size: 64 * 1024);
    esp_alloc::heap_allocator!(size: 56 * 1024);

    // Initialize the timer group required by the RTOS
    let timg0 = TimerGroup::new(peripherals.TIMG0);

    // Start the RTOS scheduler
    esp_rtos::start(timg0.timer0, peripherals.FROM_CPU_INTR0);

    let token = sensor_task(peripherals.I2C0, peripherals.GPIO8, peripherals.GPIO9)
        .expect("Failed to create sensor task");

    spawner.spawn(token);

    let network = match Network::initialize(peripherals.WIFI, &spawner).await {
        Ok(network) => network,
        Err(error) => {
            println!("FATAL: {}", error);
            loop {}
        }
    };

    if let Err(error) = network.wait_for_dhcp().await {
        println!("FATAL: {}", error);
        loop {}
    }

    let rng = sourdough_monitor::cloud::crypto_rng::initialize_tls_rng();

    let tls = match Tls::new(rng) {
        Ok(tls) => {
            println!("TLS engine initialized successfully!");
            tls
        }
        Err(error) => {
            println!("FATAL: TLS initialization failed: {:?}", error);
            loop {}
        }
    };

    let thingspeak = ThingSpeak::new(network.stack());
    let (rx_buffer, tx_buffer) = ThingSpeak::initialize_buffers();

    println!("Connecting to ThingSpeak...");

    let mut next_sample = Instant::now();
    let mut next_upload = next_sample + Duration::from_millis(UPLOAD_INTERVAL_MS);

    loop {
        let now = Instant::now();

        // Sensor sampling schedule
        if now >= next_sample {
            next_sample += Duration::from_millis(SAMPLE_INTERVAL_MS);

            // Skip missed sampling deadlines rather than taking
            // several measurements back-to-back
            if next_sample <= now {
                next_sample = now + Duration::from_millis(SAMPLE_INTERVAL_MS);
            }
        }

        // ThingSpeak upload schedule
        let now = Instant::now();

        if now >= next_upload {
            next_upload += Duration::from_millis(UPLOAD_INTERVAL_MS);

            if next_upload <= now {
                next_upload = now + Duration::from_millis(UPLOAD_INTERVAL_MS);
            }

            if let Some(readings) = SHARED_STATE.latest_readings() {
                match thingspeak
                    .upload_measurements(
                        &tls,
                        readings.relative_height_mm,
                        readings.temperature_celsius,
                        readings.humidity_percent,
                        readings.tvoc_ppb,
                        readings.co2_ppm,
                        &mut rx_buffer[..],
                        &mut tx_buffer[..],
                    )
                    .await
                {
                    Ok(entry_id) => {
                        println!("ThingSpeak upload successful: {}", entry_id);
                    }
                    Err(error) => {
                        println!("ThingSpeak upload failed: {:?}", error);
                    }
                }
            } else {
                println!("ThingSpeak upload skipped: no valid measurements yet.");
            }
        }

        // Sleep until whichever event is scheduled next
        let next_wakeup = core::cmp::min(next_sample, next_upload);
        Timer::at(next_wakeup).await;
    }
}

#[embassy_executor::task]
async fn sensor_task(i2c0: I2C0<'static>, sda: GPIO8<'static>, scl: GPIO9<'static>) {
    // Initialize the I²C peripheral.
    let i2c = I2c::new(i2c0, I2cConfig::default())
        .unwrap()
        .with_sda(sda)
        .with_scl(scl);

    // The bus and all sensor drivers live inside this task
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

    // Initialize the delay required by the sensor drivers
    let mut delay = Delay::new();

    // Establish the VL6180X reference
    match sensors.vl6180x.establish_reference(&mut delay, 2000) {
        Ok(distance) => {
            println!("VL6180X reference distance: {} mm", distance);
        }
        Err(error) => {
            println!("FATAL: Could not establish VL6180X reference: {:?}", error);
            halt();
        }
    }

    // Preserve the existing sampling schedule
    let mut next_sample = Instant::now();

    loop {
        let now = Instant::now();

        if now >= next_sample {
            next_sample += Duration::from_millis(SAMPLE_INTERVAL_MS);

            // Skip missed deadlines instead of taking measurements
            // back-to-back
            if next_sample <= now {
                next_sample = now + Duration::from_millis(SAMPLE_INTERVAL_MS);
            }

            match sensors::get_measurements(&mut sensors, &mut delay) {
                Ok(readings) => {
                    println!("Distance: {} mm", readings.relative_height_mm);
                    println!("ENS160 TVOC: {} ppb", readings.ens160.tvoc_ppb);
                    println!("AHT21: {:?}", readings.aht21);
                    println!("SCD41 CO2: {} ppm", readings.scd41.co2_ppm);

                    let latest = LatestReadings {
                        relative_height_mm: readings.relative_height_mm,
                        temperature_celsius: readings.aht21.temperature_calibrated,
                        humidity_percent: readings.aht21.humidity,
                        tvoc_ppb: readings.ens160.tvoc_ppb,
                        co2_ppm: readings.scd41.co2_ppm,
                    };

                    SHARED_STATE.update(latest);
                }
                Err(error) => {
                    println!("Measurement error: {:?}", error);
                }
            }
        }

        Timer::at(next_sample).await;
    }
}
