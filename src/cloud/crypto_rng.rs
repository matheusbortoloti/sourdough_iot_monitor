//! Adapter between the ESP32-C3 hardware RNG and rand_core.

use core::convert::Infallible;
use static_cell::StaticCell;

static TLS_RNG: StaticCell<EspHardwareRng> = StaticCell::new();

pub fn initialize_tls_rng() -> &'static mut EspHardwareRng {
    TLS_RNG.init(EspHardwareRng::new())
}

pub struct EspHardwareRng {
    rng: esp_hal::rng::Rng,
}

impl EspHardwareRng {
    pub fn new() -> Self {
        Self {
            rng: esp_hal::rng::Rng::new(),
        }
    }
}

impl rand_core::TryRng for EspHardwareRng {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok(self.rng.random())
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        let high = u64::from(self.rng.random());
        let low = u64::from(self.rng.random());

        Ok((high << 32) | low)
    }

    fn try_fill_bytes(
        &mut self,
        dest: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.rng.read(dest);
        Ok(())
    }
}

impl rand_core::TryCryptoRng for EspHardwareRng {}