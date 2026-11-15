class KalmanFilter1D:

    def __init__(self, measurement_noise: float, process_noise: float):
        self.estimate = 0.0
        self.error_estimate = 1.0
        self.process_noise = process_noise  # Q
        self.measurement_noise = measurement_noise  # R
        self.initialized = False

    def update(self, measurement_mm: float) -> float:
        if not self.initialized:
            self.estimate = float(measurement_mm)
            self.initialized = True
            return self.estimate

        # 1. Prediction step
        self.error_estimate += self.process_noise

        # 2. Measurement update (Kalman Gain)
        kalman_gain = self.error_estimate / (
            self.error_estimate + self.measurement_noise
        )
        self.estimate = self.estimate + kalman_gain * (
            measurement_mm - self.estimate
        )
        self.error_estimate = (1.0 - kalman_gain) * self.error_estimate

        return self.estimate


    if __name__ == "__main__":
        print("Include example later...")