//! 1D Kalman filter implementation for smoothing noisy scalar sensor readings

/// A lightweight, zero-allocation 1D Kalman filter
#[derive(Debug)]
pub struct KalmanFilter1D {
    estimate: f32,
    error_estimate: f32,
    process_noise: f32,
    measurement_noise: f32,
    initialized: bool,
}

impl KalmanFilter1D {
    /// Creates a new filter with the given measurement and process noise
    pub const fn new(measurement_noise: f32, process_noise: f32) -> Self {
        Self {
            estimate: 0.0,
            error_estimate: 1.0,
            process_noise,
            measurement_noise,
            initialized: false,
        }
    }

    /// Resets the internal state so the next reading re-initializes the filter
    pub fn reset(&mut self) {
        self.estimate = 0.0;
        self.error_estimate = 1.0;
        self.initialized = false;
    }

    /// Feeds a new raw measurement reading and returns the smoothed estimate
    pub fn update(&mut self, measurement_mm: i16) -> f32 {
        let meas = measurement_mm as f32;

        if !self.initialized {
            self.estimate = meas;
            self.initialized = true;
            return self.estimate;
        }

        // 1. Prediction step
        self.error_estimate += self.process_noise;

        // 2. Measurement update (Kalman Gain)
        let kalman_gain = self.error_estimate / (self.error_estimate + self.measurement_noise);
        self.estimate = self.estimate + kalman_gain * (meas - self.estimate);
        self.error_estimate = (1.0 - kalman_gain) * self.error_estimate;

        self.estimate
    }
}
