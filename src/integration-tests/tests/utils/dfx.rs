use std::process::Command;

/// Returns the local dfx url
pub fn dfx_url() -> String {
    format!("http://localhost:{}", dfx_webserver_port())
}

/// Returns local dfx replica port
fn dfx_webserver_port() -> u16 {
    dfx_info_port("webserver-port")
}

/// Returns the port of the dfx service
fn dfx_info_port(service: &str) -> u16 {
    Command::new("dfx")
        .args(["info", service])
        .output()
        .expect("Failed to get dfx port")
        .stdout
        .iter()
        .map(|&b| b as char)
        .collect::<String>()
        .trim()
        .parse::<u16>()
        .expect("Failed to parse dfx port")
}
