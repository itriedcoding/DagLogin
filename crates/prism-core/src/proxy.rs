//! Proxy testing and bulk import/export.

use crate::error::{AppError, Result};
use crate::model::{now_iso, Proxy, ProxyInput, ProxyProtocol, ProxyTest};
use std::time::{Duration, Instant};

fn encode_userinfo(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        let ch = byte as char;
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.' | '_' | '~') {
            out.push(ch);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Builds the proxy URL used by the tester (and only by the tester — the browser
/// never sees credentials, see `browser::relay`).
pub fn proxy_url(protocol: ProxyProtocol, host: &str, port: u16, username: &str, password: &str) -> String {
    let host = host.trim();
    if username.is_empty() && password.is_empty() {
        format!("{}://{}:{}", protocol.scheme(), host, port)
    } else {
        format!(
            "{}://{}:{}@{}:{}",
            protocol.scheme(),
            encode_userinfo(username),
            encode_userinfo(password),
            host,
            port
        )
    }
}

fn extract_failure(err: &reqwest::Error) -> String {
    let text = err.to_string();
    let lower = text.to_lowercase();
    if lower.contains("407") || lower.contains("proxy authentication") {
        return "the proxy rejected the credentials (407 Proxy Authentication Required)".into();
    }
    if lower.contains("timed out") || lower.contains("timeout") {
        return "the proxy did not answer in time".into();
    }
    if lower.contains("dns") {
        return "the proxy host name could not be resolved".into();
    }
    if lower.contains("connection refused") {
        return "the proxy refused the connection (is the port correct?)".into();
    }
    if lower.contains("certificate") || lower.contains("tls") {
        return format!("TLS negotiation through the proxy failed: {text}");
    }
    format!("the request through the proxy failed: {text}")
}

/// Performs a real request through the proxy and reports the exit IP so a user
/// can confirm which network the profile will appear from.
pub async fn test(
    protocol: ProxyProtocol,
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    test_url: &str,
) -> ProxyTest {
    let tested_at = now_iso();
    if host.trim().is_empty() || port == 0 {
        return ProxyTest {
            ok: false,
            latency_ms: None,
            exit_ip: None,
            country: None,
            error: Some("proxy host and port are required".into()),
            tested_at,
        };
    }
    let url = proxy_url(protocol, host, port, username, password);
    let proxy = match reqwest::Proxy::all(&url) {
        Ok(proxy) => proxy,
        Err(err) => {
            return ProxyTest {
                ok: false,
                latency_ms: None,
                exit_ip: None,
                country: None,
                error: Some(format!("invalid proxy configuration: {err}")),
                tested_at,
            }
        }
    };
    let client = match reqwest::Client::builder()
        .proxy(proxy)
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(12))
        .user_agent("Prism/1.0 proxy-check")
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            return ProxyTest {
                ok: false,
                latency_ms: None,
                exit_ip: None,
                country: None,
                error: Some(format!("could not create the test client: {err}")),
                tested_at,
            }
        }
    };

    let url = if test_url.trim().is_empty() {
        "https://api.ipify.org?format=json"
    } else {
        test_url.trim()
    };
    let started = Instant::now();
    match client.get(url).send().await {
        Ok(response) => {
            let latency = started.elapsed().as_millis() as u64;
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if !status.is_success() {
                return ProxyTest {
                    ok: false,
                    latency_ms: Some(latency),
                    exit_ip: None,
                    country: None,
                    error: Some(format!(
                        "the proxy connected but the test endpoint returned HTTP {}",
                        status.as_u16()
                    )),
                    tested_at,
                };
            }
            let (exit_ip, country) = parse_exit_info(&body);
            ProxyTest {
                ok: true,
                latency_ms: Some(latency),
                exit_ip,
                country,
                error: None,
                tested_at,
            }
        }
        Err(err) => ProxyTest {
            ok: false,
            latency_ms: Some(started.elapsed().as_millis() as u64),
            exit_ip: None,
            country: None,
            error: Some(extract_failure(&err)),
            tested_at,
        },
    }
}

fn parse_exit_info(body: &str) -> (Option<String>, Option<String>) {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
        let ip = value
            .get("ip")
            .or_else(|| value.get("query"))
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_string());
        let country = value
            .get("country")
            .or_else(|| value.get("country_code"))
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_string());
        if ip.is_some() {
            return (ip, country);
        }
    }
    let trimmed = body.trim();
    if !trimmed.is_empty() && trimmed.len() <= 64 && !trimmed.contains(char::is_whitespace) {
        (Some(trimmed.to_string()), None)
    } else {
        (None, None)
    }
}

/// Parses pasted proxy lists. Accepts the formats people actually copy around:
///
/// ```text
/// socks5://user:pass@1.2.3.4:1080
/// http://user:pass@host:8080
/// host:1080
/// host:1080:user:pass
/// user:pass@host:1080
/// label,protocol,host,port,user,pass
/// ```
pub fn parse_list(text: &str) -> Result<Vec<ProxyInput>> {
    let mut out = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        match parse_line(line) {
            Ok(proxy) => out.push(proxy),
            Err(err) => {
                return Err(AppError::validation(format!(
                    "line {}: {err}",
                    index + 1
                )))
            }
        }
    }
    if out.is_empty() {
        return Err(AppError::validation("no proxies were found in the pasted text"));
    }
    Ok(out)
}

fn parse_line(line: &str) -> Result<ProxyInput> {
    // CSV form: label,protocol,host,port,user,pass
    if line.contains(',') {
        let parts: Vec<&str> = line.split(',').map(str::trim).collect();
        if parts.len() >= 4 {
            let protocol = ProxyProtocol::parse(parts[1])?;
            let port: u16 = parts[3]
                .parse()
                .map_err(|_| AppError::validation(format!("invalid port \"{}\"", parts[3])))?;
            return Ok(ProxyInput {
                id: None,
                label: parts[0].to_string(),
                protocol,
                host: parts[2].to_string(),
                port,
                username: parts.get(4).copied().unwrap_or_default().to_string(),
                password: parts.get(5).copied().unwrap_or_default().to_string(),
            });
        }
    }

    let mut protocol = ProxyProtocol::Http;
    let mut rest = line;
    if let Some((scheme, remainder)) = line.split_once("://") {
        protocol = ProxyProtocol::parse(scheme)?;
        rest = remainder;
    }

    let (credentials, host_port) = match rest.rsplit_once('@') {
        Some((creds, host_port)) => (Some(creds), host_port),
        None => (None, rest),
    };
    let (username, password) = match credentials {
        Some(creds) => match creds.split_once(':') {
            Some((user, pass)) => (user.to_string(), pass.to_string()),
            None => (creds.to_string(), String::new()),
        },
        None => (String::new(), String::new()),
    };

    let host_port = host_port.trim_matches('/');
    let parts: Vec<&str> = host_port.split(':').collect();
    if parts.len() < 2 {
        return Err(AppError::validation(format!(
            "could not parse \"{line}\" (expected host:port)"
        )));
    }
    let host = parts[0].trim();
    let port: u16 = parts[1]
        .trim()
        .parse()
        .map_err(|_| AppError::validation(format!("invalid port \"{}\"", parts[1])))?;
    if host.is_empty() {
        return Err(AppError::validation("proxy host is empty"));
    }

    // `host:port:user:pass` with no `@`.
    let (username, password) = if username.is_empty() && parts.len() >= 3 {
        (
            parts[2].trim().to_string(),
            parts.get(3).map(|v| v.trim().to_string()).unwrap_or_default(),
        )
    } else {
        (username, password)
    };

    Ok(ProxyInput {
        id: None,
        label: format!("{host}:{port}"),
        protocol,
        host: host.to_string(),
        port,
        username,
        password,
    })
}

/// Exports proxies as a portable list. Passwords are included because this is
/// an explicit user-initiated export of their own credentials.
pub fn export_list(proxies: &[Proxy]) -> String {
    let mut out = String::from("# Prism proxy export\n# protocol://user:password@host:port\n");
    for proxy in proxies {
        out.push_str(&proxy_url(
            proxy.protocol,
            &proxy.host,
            proxy.port,
            &proxy.username,
            &proxy.password,
        ));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_url_style_entries() {
        let list = parse_list(
            "socks5://alice:s3cret@1.2.3.4:1080\nhttp://5.6.7.8:8080\n# comment\n",
        )
        .unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].protocol, ProxyProtocol::Socks5);
        assert_eq!(list[0].username, "alice");
        assert_eq!(list[0].password, "s3cret");
        assert_eq!(list[0].port, 1080);
        assert_eq!(list[1].protocol, ProxyProtocol::Http);
        assert!(list[1].username.is_empty());
    }

    #[test]
    fn parses_colon_and_at_formats() {
        let list = parse_list("1.2.3.4:1080:bob:pw\nbob:pw@5.6.7.8:9000").unwrap();
        assert_eq!(list[0].username, "bob");
        assert_eq!(list[0].password, "pw");
        assert_eq!(list[1].username, "bob");
        assert_eq!(list[1].port, 9000);
    }

    #[test]
    fn parses_csv_rows_with_labels() {
        let list = parse_list("Frankfurt,socks5,10.0.0.9,1080,user,pass").unwrap();
        assert_eq!(list[0].label, "Frankfurt");
        assert_eq!(list[0].protocol, ProxyProtocol::Socks5);
        assert_eq!(list[0].password, "pass");
    }

    #[test]
    fn rejects_garbage_with_the_line_number() {
        let err = parse_list("1.2.3.4:1080\nnot-a-proxy\n").unwrap_err();
        assert!(err.to_string().contains("line 2"), "{err}");
    }

    #[test]
    fn escape_userinfo_for_urlish_proxies() {
        assert_eq!(encode_userinfo("p@ss:word"), "p%40ss%3Aword");
    }

    #[test]
    fn parses_ip_and_country_from_json_and_plain_text() {
        assert_eq!(
            parse_exit_info("{\"ip\":\"9.9.9.9\",\"country\":\"DE\"}"),
            (Some("9.9.9.9".into()), Some("DE".into()))
        );
        assert_eq!(
            parse_exit_info("9.9.9.9\n"),
            (Some("9.9.9.9".into()), None)
        );
        assert_eq!(parse_exit_info("<html>oops</html>"), (None, None));
    }

    #[test]
    fn export_round_trips_through_the_parser() {
        let proxy = Proxy {
            id: "1".into(),
            label: "x".into(),
            protocol: ProxyProtocol::Socks5,
            host: "1.2.3.4".into(),
            port: 1080,
            username: "u".into(),
            password: "p@ss".into(),
            created_at: String::new(),
            updated_at: String::new(),
            last_test: None,
        };
        let text = export_list(&[proxy]);
        let parsed = parse_list(&text).unwrap();
        assert_eq!(parsed[0].password, "p@ss");
        assert_eq!(parsed[0].port, 1080);
    }
}
