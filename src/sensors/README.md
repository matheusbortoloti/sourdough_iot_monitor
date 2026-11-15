# Sensors

## VL6180X

This Time-of-Flight (TOF) sensor has a lot of noise. Only using the standard sensor configurations and no filtering does not yield good results.
In order to compensate this high noise we adjust the sensor configurations and also implement some digital filtering on the ESP32:

1. **Increase Readout Averaging:** the internal register `READOUT__AVERAGING_SAMPLE_PERIOD (0x10A)` has a default value of 48, which corresponds to 4.3 ms.
We can increase this value up to 255 to tell the sensor to use more samples for a single shot measurement. 
2. **One-dimensional Kalman Filter**: since we can calculate the measurement noise from the raw measurements, we can parametrize a 1D Kalman filter to remove this noise from the measurement data.

In order to estimate the measurement noise, a static distance of aroud 34 mm has been measured multiple times.
The table bellow shows quantitatively the improvements obtained with the actions described above.


|  Filtering Mode                                             | Mean (mm)                      | Standard Deviation (mm)       |
| :---------------------------------------------------------- | :----------------------------: | :---------------------------: | 
| $\color{red}{\text{Raw Measurements}}$                      | $\color{red}{\text{35.80}}$    | $\color{red}{\text{2.54}}$    | 
| $\color{orange}{\text{Increased Averaging}}$                | $\color{orange}{\text{34.14}}$ | $\color{orange}{\text{1.14}}$ | 
| $\color{green}{\text{Increased Averaging + Kalman Filter}}$ | $\color{green}{\text{34.14}}$  | $\color{green}{\text{0.11}}$  | 


The picture bellows shows graphically the improvement obtained between the raw data and the data with increased averaging plus the Kalman filter.

![VL6180X Raw Measurements](../../doc/pictures/vl6180x_measurements.png)
