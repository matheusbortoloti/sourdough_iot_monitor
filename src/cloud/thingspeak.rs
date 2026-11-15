//! ThingSpeak HTTPS communication.

use core::fmt::{self, Write as FmtWrite};
use embassy_net::IpEndpoint;
use embassy_net::Stack;
use embassy_net::tcp::TcpSocket;
use esp_println::println;
use mbedtls_rs::io::Write;
use mbedtls_rs::{Session, SessionConfig, Tls};
use static_cell::StaticCell;

const THINGSPEAK_WRITE_API_KEY: &str = env!("THINGSPEAK_WRITE_API_KEY");

static TCP_RX_BUFFER: StaticCell<[u8; 4096]> = StaticCell::new();
static TCP_TX_BUFFER: StaticCell<[u8; 4096]> = StaticCell::new();

#[derive(Debug)]
pub enum ThingSpeakError {
    Dns(embassy_net::dns::Error),
    Tcp(embassy_net::tcp::ConnectError),
    TlsConfig,
    Tls(mbedtls_rs::SessionError),
    RequestBuffer,
    ResponseBuffer,
    InvalidResponse,
    HttpStatus(u16),
    UpdateRejected,
}

pub struct ThingSpeak {
    stack: Stack<'static>,
}

struct FixedBuffer<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> FixedBuffer<N> {
    fn new() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
        }
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    fn as_str(&self) -> Result<&str, ThingSpeakError> {
        core::str::from_utf8(self.as_bytes()).map_err(|_| ThingSpeakError::InvalidResponse)
    }
}

impl<const N: usize> fmt::Write for FixedBuffer<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len + s.len();

        if end > N {
            return Err(fmt::Error);
        }

        self.bytes[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;

        Ok(())
    }
}

impl ThingSpeak {
    pub fn new(stack: Stack<'static>) -> Self {
        Self { stack }
    }

    pub async fn resolve_server(&self) -> Result<embassy_net::IpAddress, embassy_net::dns::Error> {
        let addresses = self
            .stack
            .dns_query("api.thingspeak.com", embassy_net::dns::DnsQueryType::A)
            .await?;

        addresses
            .first()
            .copied()
            .ok_or(embassy_net::dns::Error::Failed)
    }

    pub async fn connect<'a>(
        &self,
        rx_buffer: &'a mut [u8],
        tx_buffer: &'a mut [u8],
    ) -> Result<TcpSocket<'a>, ThingSpeakError> {
        let mut socket = TcpSocket::new(self.stack, rx_buffer, tx_buffer);

        socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

        let address = self.resolve_server().await.map_err(ThingSpeakError::Dns)?;

        socket
            .connect(IpEndpoint::new(address, 443))
            .await
            .map_err(ThingSpeakError::Tcp)?;

        Ok(socket)
    }

    pub fn initialize_buffers() -> (&'static mut [u8; 4096], &'static mut [u8; 4096]) {
        (TCP_RX_BUFFER.init([0; 4096]), TCP_TX_BUFFER.init([0; 4096]))
    }

    pub async fn establish_tls(
        &self,
        tls: &Tls<'_>,
        rx_buffer: &'static mut [u8],
        tx_buffer: &'static mut [u8],
    ) -> Result<(), ThingSpeakError> {
        let socket = self.connect(rx_buffer, tx_buffer).await?;

        let client_config = crate::cloud::tls::client_config().ok_or(ThingSpeakError::TlsConfig)?;

        let config = SessionConfig::Client(client_config);

        println!("Heap before TLS session creation:");
        println!("{}", esp_alloc::HEAP.stats());

        let mut session =
            Session::new(tls.reference(), socket, &config).map_err(ThingSpeakError::Tls)?;

        println!("TLS session created successfully.");

        session.connect().await.map_err(ThingSpeakError::Tls)?;

        println!("TLS handshake successful!");
        println!("Heap after TLS handshake:");
        println!("{}", esp_alloc::HEAP.stats());

        Ok(())
    }

    pub async fn upload_measurements(
        &self,
        tls: &Tls<'_>,
        relative_height_mm: f32,
        temperature: f32,
        humidity: f32,
        tvoc_ppb: u16,
        rx_buffer: &mut [u8],
        tx_buffer: &mut [u8],
    ) -> Result<u32, ThingSpeakError> {
        let socket = self.connect(rx_buffer, tx_buffer).await?;

        let client_config = crate::cloud::tls::client_config().ok_or(ThingSpeakError::TlsConfig)?;

        let config = SessionConfig::Client(client_config);

        let mut session =
            Session::new(tls.reference(), socket, &config).map_err(ThingSpeakError::Tls)?;

        session.connect().await.map_err(ThingSpeakError::Tls)?;

        println!("ThingSpeak TLS handshake successful!");

        // Build the form-encoded POST body.
        let mut body = FixedBuffer::<128>::new();

        write!(
            &mut body,
            "api_key={}&field1={}&field2={:.2}&field3={:.2}&field4={}",
            THINGSPEAK_WRITE_API_KEY, relative_height_mm, temperature, humidity, tvoc_ppb,
        )
        .map_err(|_| ThingSpeakError::RequestBuffer)?;

        // Build the HTTP request.
        let mut request = FixedBuffer::<384>::new();

        write!(
            &mut request,
            "POST /update HTTP/1.1\r\n\
            Host: api.thingspeak.com\r\n\
            Content-Type: application/x-www-form-urlencoded\r\n\
            Content-Length: {}\r\n\
            Connection: close\r\n\
            \r\n",
            body.len,
        )
        .map_err(|_| ThingSpeakError::RequestBuffer)?;

        request
            .write_str(body.as_str()?)
            .map_err(|_| ThingSpeakError::RequestBuffer)?;

        // Send the request.
        session
            .write_all(request.as_bytes())
            .await
            .map_err(ThingSpeakError::Tls)?;

        session.flush().await.map_err(ThingSpeakError::Tls)?;

        println!("ThingSpeak POST request sent.");

        // Receive the HTTP response.
        let mut response = [0u8; 2048];
        let mut response_len = 0;

        let mut chunk = [0u8; 128];
        let mut body_start = None;
        let mut content_length = None;

        loop {
            let n = session
                .read(&mut chunk)
                .await
                .map_err(ThingSpeakError::Tls)?;

            if n == 0 {
                break;
            }

            if response_len + n > response.len() {
                return Err(ThingSpeakError::ResponseBuffer);
            }

            response[response_len..response_len + n].copy_from_slice(&chunk[..n]);

            response_len += n;

            let response_text = core::str::from_utf8(&response[..response_len])
                .map_err(|_| ThingSpeakError::InvalidResponse)?;

            if body_start.is_none() {
                if let Some(position) = response_text.find("\r\n\r\n") {
                    body_start = Some(position + 4);

                    for line in response_text[..position].split("\r\n").skip(1) {
                        if let Some(value) = line
                            .strip_prefix("Content-Length: ")
                            .or_else(|| line.strip_prefix("content-length: "))
                        {
                            content_length = value.parse::<usize>().ok();
                        }
                    }
                }
            }

            if let (Some(start), Some(length)) = (body_start, content_length) {
                if response_len >= start + length {
                    break;
                }
            }
        }

        let response_text = core::str::from_utf8(&response[..response_len])
            .map_err(|_| ThingSpeakError::InvalidResponse)?;

        let status_line = response_text
            .split("\r\n")
            .next()
            .ok_or(ThingSpeakError::InvalidResponse)?;

        let status_code: u16 = status_line
            .split_ascii_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .ok_or(ThingSpeakError::InvalidResponse)?;

        if status_code != 200 {
            return Err(ThingSpeakError::HttpStatus(status_code));
        }

        let body_start = response_text
            .find("\r\n\r\n")
            .ok_or(ThingSpeakError::InvalidResponse)?;

        let entry_id: u32 = response_text[body_start + 4..]
            .trim()
            .parse()
            .map_err(|_| ThingSpeakError::InvalidResponse)?;

        if entry_id == 0 {
            return Err(ThingSpeakError::UpdateRejected);
        }

        println!("ThingSpeak update successful! Entry ID: {}", entry_id);

        Ok(entry_id)
    }
}
