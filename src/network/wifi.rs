//! Wi-Fi and network connectivity management.
use core::fmt;
use embassy_executor::Spawner;
use embassy_net::{Runner, Stack, StackResources};
use embassy_time::{Duration, with_timeout};
use esp_hal::peripherals::WIFI;
use esp_println::println;
use esp_radio::wifi::{
    AuthenticationMethodConfig, Config as WifiConfig, Interface, WifiController, scan::ScanConfig,
    sta::StationConfig,
};
use esp_radio::wifi::{ConnectionError, WifiError};
use static_cell::StaticCell;

pub struct Network {
    _controller: WifiController<'static>,
    stack: Stack<'static>,
}

/// Errors that can occur during network initialization.
/// Errors that can occur during network initialization.
#[derive(Debug)]
pub enum NetworkError {
    WifiInitialization(WifiError),
    WifiConfiguration(WifiError),
    WifiConnection(ConnectionError),
    InvalidCredentials,
    TaskSpawn,
    DhcpTimeout,
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WifiInitialization(error) => {
                write!(f, "Wi-Fi initialization failed: {:?}", error)
            }
            Self::WifiConfiguration(error) => {
                write!(f, "Wi-Fi configuration failed: {:?}", error)
            }
            Self::WifiConnection(error) => {
                write!(f, "Wi-Fi connection failed: {:?}", error)
            }
            Self::InvalidCredentials => {
                write!(f, "Invalid Wi-Fi credentials")
            }
            Self::TaskSpawn => {
                write!(f, "Failed to create network background task")
            }
            Self::DhcpTimeout => {
                write!(f, "DHCP configuration timed out")
            }
        }
    }
}

const WIFI_SSID: &str = env!("WIFI_SSID");
const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, Interface>) {
    runner.run().await
}

impl Network {
    pub async fn initialize(
        wifi_peripheral: WIFI<'static>,
        spawner: &Spawner,
    ) -> Result<Self, NetworkError> {
        let controller = connect(wifi_peripheral).await?;

        let stack = start_network(spawner)?;

        Ok(Self {
            _controller: controller,
            stack,
        })
    }

    pub async fn wait_for_dhcp(&self) -> Result<(), NetworkError> {
        println!("Waiting for DHCP...");

        // Wait up to 30 seconds for network configuration.
        with_timeout(Duration::from_secs(30), self.stack.wait_config_up())
            .await
            .map_err(|_| {
                println!("DHCP configuration timed out!");
                NetworkError::DhcpTimeout
            })?;

        println!("Network configured successfully!");

        if let Some(config) = self.stack.config_v4() {
            println!("IP address: {}", config.address);
            println!("Gateway: {:?}", config.gateway);
        }

        Ok(())
    }

    pub fn stack(&self) -> Stack<'static> {
        self.stack
    }
}

pub async fn connect(
    wifi_peripheral: esp_hal::peripherals::WIFI<'static>,
) -> Result<WifiController<'static>, NetworkError> {
    // Initialize the Wi-Fi controller.
    let mut controller = match WifiController::new(wifi_peripheral, Default::default()) {
        Ok(controller) => {
            println!("Wi-Fi controller initialized successfully!");
            controller
        }
        Err(error) => {
            return Err(NetworkError::WifiInitialization(error));
        }
    };

    // Scan for nearby Wi-Fi networks.
    println!("Scanning for Wi-Fi networks...");

    let scan_config = ScanConfig::default().with_max(20);

    match controller.scan_async(&scan_config).await {
        Ok(access_points) => {
            println!("Found {} access points:", access_points.len());
        }
        Err(error) => {
            println!("Wi-Fi scan failed: {:?}", error);
        }
    }

    // Configure Wi-Fi station mode.
    println!("Configuring Wi-Fi station...");
    // Convert Wi-Fi credentials into the types required by esp-radio.
    let ssid = WIFI_SSID
        .try_into()
        .map_err(|_| NetworkError::InvalidCredentials)?;

    let password = WIFI_PASSWORD
        .try_into()
        .map_err(|_| NetworkError::InvalidCredentials)?;

    let station_config = WifiConfig::Station(
        StationConfig::default()
            .with_ssid(ssid)
            .with_authentication(AuthenticationMethodConfig::Wpa2Personal(password)),
    );

    match controller.set_config(&station_config) {
        Ok(()) => println!("Wi-Fi station configured successfully!"),
        Err(error) => {
            return Err(NetworkError::WifiConfiguration(error));
        }
    }

    // Connect to the access point.
    println!("Connecting to Wi-Fi...");

    match controller.connect_async().await {
        Ok(_connection_info) => {
            println!("Wi-Fi connection established!");
        }
        Err(error) => {
            return Err(NetworkError::WifiConnection(error));
        }
    }

    Ok(controller)
}

pub fn start_network(spawner: &Spawner) -> Result<Stack<'static>, NetworkError> {
    // Create the Wi-Fi station network interface.
    let wifi_interface = Interface::station();

    println!("Wi-Fi station interface created!");
    println!("Station MAC address: {:02X?}", wifi_interface.mac_address());

    // Configure DHCP.
    let net_config = embassy_net::Config::dhcpv4(Default::default());

    // Allocate static memory for network resources.
    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
    let resources = RESOURCES.init(StackResources::new());

    // Generate a random seed using the ESP32-C3 hardware RNG.
    let rng = esp_hal::rng::Rng::new();

    let random_seed = (u64::from(rng.random()) << 32) | u64::from(rng.random());

    // Create the network stack and background runner.
    let (stack, runner) = embassy_net::new(wifi_interface, net_config, resources, random_seed);

    println!("Embassy Net stack created!");

    // Create the background task.
    let task = net_task(runner).map_err(|error| {
        println!("Failed to create network task: {:?}", error);
        NetworkError::TaskSpawn
    })?;

    // Start the background task.
    spawner.spawn(task);

    println!("Network background task started!");

    Ok(stack)
}
