# IoT Sourdough Monitor

![Schematics](doc/pictures/schematics.drawio.png)


Repository structure

```
sourdough_monitor/
│
├── .cargo/
│   └── config.toml
│
├── src/
│   ├── bin/
│   │   └── main.rs
│   │
│   ├── sensors/
│   │   ├── mod.rs
│   │   ├── aht21.rs
│   │   ├── ens160.rs
│   │   └── vl6180x.rs
│   │
│   ├── network/
│   │   └── mod.rs
│   │
│   └── lib.rs
│
├── .gitignore
├── Cargo.lock
├── Cargo.toml
├── LICENSE
└── README.md
```

# I2C Architecture

```
ESP32-C3
   |
   | I2C Controller
   |
   +---- VL6180X driver
   |
   +---- ENS160 driver
   |
   +---- AHT21 driver
```