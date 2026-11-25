//! Local proxy relay.
//!
//! Chromium's `--proxy-server` switch cannot carry credentials, and its SOCKS5
//! client does not implement username/password authentication at all. Real
//! multi-account browsers solve this with a loopback relay: the browser talks
//! unauthenticated SOCKS5 to `127.0.0.1:<port>`, and the relay performs the
//! authenticated handshake with the upstream proxy (HTTP CONNECT, HTTPS CONNECT
//! or SOCKS5 with RFC 1929 auth) before piping bytes.
//!
//! Consequences:
//!   * proxy passwords never reach the browser process or its profile files;
//!   * one relay per running profile means two profiles can use two different
//!     proxies at the same time;
//!   * the relay dies with the profile, so nothing is left listening.

use crate::error::{AppError, Result};
use crate::model::ProxyProtocol;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(20);

pub trait AsyncIo: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> AsyncIo for T {}
type Stream = Box<dyn AsyncIo>;

#[derive(Clone, Debug)]
pub struct Upstream {
    pub protocol: ProxyProtocol,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

impl Upstream {
    pub fn has_auth(&self) -> bool {
        !self.username.is_empty() || !self.password.is_empty()
    }
}

/// Handle to a running relay. Dropping it stops the listener.
pub struct Relay {
    port: u16,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl Relay {
    pub fn port(&self) -> u16 {
        self.port
    }

    /// `socks5://127.0.0.1:<port>` — what the browser is launched with.
    pub fn browser_proxy(&self) -> String {
        format!("socks5://127.0.0.1:{}", self.port)
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

pub async fn start(upstream: Upstream) -> Result<Relay> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|e| AppError::Network(format!("could not open the local proxy relay: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| AppError::Network(e.to_string()))?
        .port();
    let (tx, mut rx) = oneshot::channel::<()>();

    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut rx => break,
                accepted = listener.accept() => {
                    match accepted {
                        Ok((socket, _peer)) => {
                            let upstream = upstream.clone();
                            tokio::spawn(async move {
                                if let Err(err) = handle_client(socket, upstream).await {
                                    tracing::debug!(error = %err, "relay connection finished with an error");
                                }
                            });
                        }
                        Err(err) => {
                            tracing::warn!(error = %err, "relay accept failed");
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                    }
                }
            }
        }
    });

    Ok(Relay {
        port,
        shutdown: Some(tx),
        task: Some(task),
    })
}

async fn handle_client(mut client: TcpStream, upstream: Upstream) -> Result<()> {
    let (host, port) = match tokio::time::timeout(HANDSHAKE_TIMEOUT, socks5_greeting(&mut client))
        .await
    {
        Ok(result) => result?,
        Err(_) => return Err(AppError::Network("the browser did not complete the SOCKS5 greeting".into())),
    };

    let mut remote = connect_upstream(&upstream, &host, port).await?;

    // SOCKS5 success reply: version, status, reserved, address type, addr, port.
    client
        .write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0])
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;
    client.flush().await.ok();

    tokio::io::copy_bidirectional(&mut client, &mut remote)
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;
    Ok(())
}

/// Reads the SOCKS5 greeting + CONNECT request from the browser.
async fn socks5_greeting(client: &mut TcpStream) -> Result<(String, u16)> {
    let mut head = [0u8; 2];
    client.read_exact(&mut head).await?;
    if head[0] != 0x05 {
        let _ = client.write_all(&[0x05, 0xff]).await;
        return Err(AppError::Network("unexpected SOCKS version".into()));
    }
    let mut methods = vec![0u8; head[1] as usize];
    client.read_exact(&mut methods).await?;
    if !methods.contains(&0x00) {
        let _ = client.write_all(&[0x05, 0xff]).await;
        return Err(AppError::Network("the browser requires an unsupported SOCKS auth method".into()));
    }
    client.write_all(&[0x05, 0x00]).await?;

    let mut request = [0u8; 4];
    client.read_exact(&mut request).await?;
    if request[1] != 0x01 {
        let _ = client.write_all(&[0x05, 0x07, 0x00, 0x01, 0, 0, 0, 0, 0, 0]).await;
        return Err(AppError::Network("only SOCKS5 CONNECT is supported".into()));
    }

    let host = match request[3] {
        0x01 => {
            let mut buf = [0u8; 4];
            client.read_exact(&mut buf).await?;
            std::net::Ipv4Addr::from(buf).to_string()
        }
        0x03 => {
            let mut len = [0u8; 1];
            client.read_exact(&mut len).await?;
            let mut buf = vec![0u8; len[0] as usize];
            client.read_exact(&mut buf).await?;
            String::from_utf8(buf)
                .map_err(|_| AppError::Network("invalid host name in the SOCKS request".into()))?
        }
        0x04 => {
            let mut buf = [0u8; 16];
            client.read_exact(&mut buf).await?;
            std::net::Ipv6Addr::from(buf).to_string()
        }
        _ => {
            let _ = client.write_all(&[0x05, 0x08, 0x00, 0x01, 0, 0, 0, 0, 0, 0]).await;
            return Err(AppError::Network("unsupported SOCKS address type".into()));
        }
    };
    let mut port = [0u8; 2];
    client.read_exact(&mut port).await?;
    Ok((host, u16::from_be_bytes(port)))
}

async fn connect_upstream(upstream: &Upstream, host: &str, port: u16) -> Result<Stream> {
    let address = format!("{}:{}", upstream.host, upstream.port);
    let socket = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(&address))
        .await
        .map_err(|_| AppError::Network(format!("timed out connecting to proxy {address}")))?
        .map_err(|e| AppError::Network(format!("cannot reach proxy {address}: {e}")))?;
    socket.set_nodelay(true).ok();

    match upstream.protocol {
        ProxyProtocol::Http => {
            let mut stream: Stream = Box::new(socket);
            http_connect(&mut stream, upstream, host, port).await?;
            Ok(stream)
        }
        ProxyProtocol::Https => {
            let tls = tls_connect(socket, &upstream.host).await?;
            let mut stream: Stream = tls;
            http_connect(&mut stream, upstream, host, port).await?;
            Ok(stream)
        }
        ProxyProtocol::Socks5 => {
            let mut stream: Stream = Box::new(socket);
            socks5_connect(&mut stream, upstream, host, port).await?;
            Ok(stream)
        }
    }
}

async fn tls_connect(socket: TcpStream, host: &str) -> Result<Stream> {
    // Uses the platform TLS stack (SChannel on Windows), so an HTTPS proxy is
    // validated against the machine's own trust store.
    let connector = native_tls::TlsConnector::builder()
        .build()
        .map_err(|e| AppError::Network(format!("TLS could not be initialised: {e}")))?;
    let connector = tokio_native_tls::TlsConnector::from(connector);
    let stream = connector
        .connect(host, socket)
        .await
        .map_err(|e| AppError::Network(format!("TLS handshake with proxy {host} failed: {e}")))?;
    Ok(Box::new(stream))
}

async fn http_connect(
    stream: &mut Stream,
    upstream: &Upstream,
    host: &str,
    port: u16,
) -> Result<()> {
    let target = format!("{host}:{port}");
    let mut request = format!(
        "CONNECT {target} HTTP/1.1\r\nHost: {target}\r\nProxy-Connection: Keep-Alive\r\n"
    );
    if upstream.has_auth() {
        let token = B64.encode(format!("{}:{}", upstream.username, upstream.password));
        request.push_str(&format!("Proxy-Authorization: Basic {token}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).await?;
    stream.flush().await.ok();

    let response = read_headers(stream).await?;
    let status = response.lines().next().unwrap_or_default().to_string();
    if status.contains(" 200") {
        return Ok(());
    }
    if status.contains(" 407") {
        return Err(AppError::Network(
            "the proxy rejected the supplied username and password (407)".into(),
        ));
    }
    Err(AppError::Network(format!(
        "the proxy refused the tunnel: {}",
        if status.is_empty() { "empty response" } else { &status }
    )))
}

async fn read_headers(stream: &mut Stream) -> Result<String> {
    let mut buffer = Vec::with_capacity(512);
    let mut byte = [0u8; 1];
    loop {
        let read = stream.read(&mut byte).await?;
        if read == 0 {
            break;
        }
        buffer.push(byte[0]);
        if buffer.len() >= 4 && &buffer[buffer.len() - 4..] == b"\r\n\r\n" {
            break;
        }
        if buffer.len() > 16 * 1024 {
            return Err(AppError::Network("the proxy sent an oversized response".into()));
        }
    }
    Ok(String::from_utf8_lossy(&buffer).to_string())
}

async fn socks5_connect(
    stream: &mut Stream,
    upstream: &Upstream,
    host: &str,
    port: u16,
) -> Result<()> {
    let want_auth = upstream.has_auth();
    let greeting: [u8; 4] = if want_auth {
        [0x05, 0x02, 0x00, 0x02]
    } else {
        [0x05, 0x01, 0x00, 0x00]
    };
    let greeting_len = if want_auth { 4 } else { 3 };
    stream.write_all(&greeting[..greeting_len]).await?;

    let mut choice = [0u8; 2];
    stream.read_exact(&mut choice).await?;
    if choice[0] != 0x05 {
        return Err(AppError::Network("the SOCKS5 proxy replied with an invalid version".into()));
    }
    match choice[1] {
        0x00 => {}
        0x02 => {
            if !want_auth {
                return Err(AppError::Network(
                    "the SOCKS5 proxy requires authentication but no credentials are configured".into(),
                ));
            }
            let user = upstream.username.as_bytes();
            let pass = upstream.password.as_bytes();
            if user.len() > 255 || pass.len() > 255 {
                return Err(AppError::validation("SOCKS5 credentials are too long"));
            }
            let mut auth = vec![0x01, user.len() as u8];
            auth.extend_from_slice(user);
            auth.push(pass.len() as u8);
            auth.extend_from_slice(pass);
            stream.write_all(&auth).await?;
            let mut status = [0u8; 2];
            stream.read_exact(&mut status).await?;
            if status[1] != 0x00 {
                return Err(AppError::Network(
                    "the SOCKS5 proxy rejected the supplied username and password".into(),
                ));
            }
        }
        0xff => {
            return Err(AppError::Network(
                "the SOCKS5 proxy does not accept the offered authentication methods".into(),
            ))
        }
        other => {
            return Err(AppError::Network(format!(
                "the SOCKS5 proxy selected an unsupported auth method ({other})"
            )))
        }
    }

    let mut request = vec![0x05, 0x01, 0x00];
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => {
            request.push(0x01);
            request.extend_from_slice(&ip.octets());
        }
        Ok(std::net::IpAddr::V6(ip)) => {
            request.push(0x04);
            request.extend_from_slice(&ip.octets());
        }
        Err(_) => {
            let bytes = host.as_bytes();
            if bytes.len() > 255 {
                return Err(AppError::Network("target host name is too long".into()));
            }
            request.push(0x03);
            request.push(bytes.len() as u8);
            request.extend_from_slice(bytes);
        }
    }
    request.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&request).await?;

    let mut reply = [0u8; 4];
    stream.read_exact(&mut reply).await?;
    if reply[1] != 0x00 {
        let reason = match reply[1] {
            0x01 => "general SOCKS server failure",
            0x02 => "connection not allowed by ruleset",
            0x03 => "network unreachable",
            0x04 => "host unreachable",
            0x05 => "connection refused",
            0x06 => "TTL expired",
            0x07 => "command not supported",
            0x08 => "address type not supported",
            _ => "unknown SOCKS5 error",
        };
        return Err(AppError::Network(format!("SOCKS5 proxy error: {reason}")));
    }
    // Drain the bound address so the tunnel starts at a clean byte offset.
    match reply[3] {
        0x01 => {
            let mut skip = [0u8; 4 + 2];
            stream.read_exact(&mut skip).await?;
        }
        0x03 => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len).await?;
            let mut skip = vec![0u8; len[0] as usize + 2];
            stream.read_exact(&mut skip).await?;
        }
        0x04 => {
            let mut skip = [0u8; 16 + 2];
            stream.read_exact(&mut skip).await?;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn accepts_protocols_that_need_credentials() {
        let upstream = Upstream {
            protocol: ProxyProtocol::Socks5,
            host: "1.2.3.4".into(),
            port: 1080,
            username: "u".into(),
            password: "p".into(),
        };
        assert!(upstream.has_auth());
    }

    /// End-to-end check: a fake SOCKS5 client talks to the relay, the relay
    /// authenticates against a fake upstream SOCKS5 proxy that requires
    /// credentials, and data flows both ways.
    #[tokio::test]
    async fn relays_through_an_authenticated_socks5_upstream() {
        let upstream_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let upstream_port = upstream_listener.local_addr().unwrap().port();

        let echo = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let echo_port = echo.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = echo.accept().await else { break };
                tokio::spawn(async move {
                    let (mut r, mut w) = socket.split();
                    let _ = tokio::io::copy(&mut r, &mut w).await;
                });
            }
        });

        tokio::spawn(async move {
            let (mut socket, _) = upstream_listener.accept().await.unwrap();
            let mut head = [0u8; 2];
            socket.read_exact(&mut head).await.unwrap();
            let mut methods = vec![0u8; head[1] as usize];
            socket.read_exact(&mut methods).await.unwrap();
            assert!(methods.contains(&0x02));
            socket.write_all(&[0x05, 0x02]).await.unwrap();
            let mut auth_head = [0u8; 2];
            socket.read_exact(&mut auth_head).await.unwrap();
            let mut user = vec![0u8; auth_head[1] as usize];
            socket.read_exact(&mut user).await.unwrap();
            let mut plen = [0u8; 1];
            socket.read_exact(&mut plen).await.unwrap();
            let mut pass = vec![0u8; plen[0] as usize];
            socket.read_exact(&mut pass).await.unwrap();
            assert_eq!(String::from_utf8(user).unwrap(), "relay-user");
            assert_eq!(String::from_utf8(pass).unwrap(), "relay-pass");
            socket.write_all(&[0x01, 0x00]).await.unwrap();

            let mut request = [0u8; 4];
            socket.read_exact(&mut request).await.unwrap();
            assert_eq!(request[1], 0x01);
            let host = match request[3] {
                0x01 => {
                    let mut buf = [0u8; 4];
                    socket.read_exact(&mut buf).await.unwrap();
                    std::net::Ipv4Addr::from(buf).to_string()
                }
                0x03 => {
                    let mut len = [0u8; 1];
                    socket.read_exact(&mut len).await.unwrap();
                    let mut buf = vec![0u8; len[0] as usize];
                    socket.read_exact(&mut buf).await.unwrap();
                    String::from_utf8(buf).unwrap()
                }
                other => panic!("unexpected SOCKS5 address type {other}"),
            };
            assert_eq!(host, "127.0.0.1", "relay must forward the requested host");
            let mut port = [0u8; 2];
            socket.read_exact(&mut port).await.unwrap();
            assert_eq!(
                u16::from_be_bytes(port),
                echo_port,
                "relay must forward the requested port"
            );
            socket
                .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await
                .unwrap();

            let mut buffer = [0u8; 4];
            socket.read_exact(&mut buffer).await.unwrap();
            socket.write_all(&buffer).await.unwrap();
        });

        let relay = start(Upstream {
            protocol: ProxyProtocol::Socks5,
            host: "127.0.0.1".into(),
            port: upstream_port,
            username: "relay-user".into(),
            password: "relay-pass".into(),
        })
        .await
        .unwrap();

        let mut client = TcpStream::connect(("127.0.0.1", relay.port())).await.unwrap();
        client.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
        let mut choice = [0u8; 2];
        client.read_exact(&mut choice).await.unwrap();
        assert_eq!(choice, [0x05, 0x00]);

        client
            .write_all(&[0x05, 0x01, 0x00, 0x01, 127, 0, 0, 1])
            .await
            .unwrap();
        client.write_all(&echo_port.to_be_bytes()).await.unwrap();
        let mut reply = [0u8; 10];
        client.read_exact(&mut reply).await.unwrap();
        assert_eq!(reply[1], 0x00);

        client.write_all(b"ping").await.unwrap();
        let mut echoed = [0u8; 4];
        client.read_exact(&mut echoed).await.unwrap();
        assert_eq!(&echoed, b"ping");
    }
}
