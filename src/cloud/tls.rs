//! TLS certificate configuration for ThingSpeak.

use core::ffi::CStr;

use mbedtls_rs::{AuthMode, Certificate, ClientSessionConfig, X509};

pub fn client_config() -> Option<ClientSessionConfig<'static>> {
    let pem = CStr::from_bytes_with_nul(
        concat!(
            include_str!("../../certs/digicert_global_root_g2.pem"),
            "\0"
        )
        .as_bytes(),
    )
    .unwrap();

    let ca = Certificate::new(X509::PEM(pem)).ok()?;

    let server_name = CStr::from_bytes_with_nul(b"api.thingspeak.com\0").unwrap();

    Some(ClientSessionConfig {
        ca_chain: Some(ca),
        server_name: Some(server_name),
        auth_mode: AuthMode::Required,
        skip_hostname_verification: false,
        ..Default::default()
    })
}
