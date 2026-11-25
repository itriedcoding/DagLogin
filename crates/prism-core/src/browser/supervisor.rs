//! Chrome DevTools Protocol supervisor.
//!
//! A stock Chromium binary cannot be told "pretend the machine is in Berlin"
//! on the command line, but it can be told through CDP: Chrome officially
//! supports timezone, locale, user-agent, screen-metric, media and hardware
//! overrides. Prism attaches to every page target of a profile's browser and
//! applies that profile's values, including for tabs opened later.

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overrides {
    pub timezone: String,
    pub locale: String,
    pub accept_language: String,
    pub user_agent: String,
    pub platform: String,
    pub width: u32,
    pub height: u32,
    pub screen_width: u32,
    pub screen_height: u32,
    pub device_scale_factor: f64,
    pub mobile: bool,
    pub color_scheme: String,
    pub hardware_concurrency: u32,
}

/// Waits for the debug endpoint of a freshly launched browser to answer.
pub async fn wait_for_cdp(port: u16, timeout: Duration) -> Option<String> {
    let deadline = tokio::time::Instant::now() + timeout;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .no_proxy()
        .build()
        .ok()?;
    let url = format!("http://127.0.0.1:{port}/json/version");
    loop {
        if tokio::time::Instant::now() > deadline {
            return None;
        }
        if let Ok(response) = client.get(&url).send().await {
            if let Ok(body) = response.json::<Value>().await {
                if let Some(ws) = body.get("webSocketDebuggerUrl").and_then(|v| v.as_str()) {
                    return Some(ws.to_string());
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
}

/// Gracefully closes a running browser through CDP. Chromium flushes cookies,
/// local storage and history before exiting, which keeps a profile consistent.
pub async fn close_browser(port: u16) -> bool {
    use futures_util::SinkExt;
    let Some(url) = wait_for_cdp(port, Duration::from_secs(2)).await else {
        return false;
    };
    let Ok((mut ws, _)) = tokio_tungstenite::connect_async(url).await else {
        return false;
    };
    let payload = json!({"id": 1, "method": "Browser.close"}).to_string();
    ws.send(Message::Text(payload.into())).await.is_ok()
}

#[derive(Clone, Copy)]
enum Tag {
    Targets,
    Attach,
}

struct Session {
    next_id: u64,
    pending: HashMap<u64, Tag>,
    overrides: Overrides,
}

impl Session {
    fn command(
        &mut self,
        method: &str,
        params: Value,
        session: Option<&str>,
        tag: Option<Tag>,
    ) -> String {
        self.next_id += 1;
        let id = self.next_id;
        if let Some(tag) = tag {
            self.pending.insert(id, tag);
        }
        let mut payload = json!({"id": id, "method": method, "params": params});
        if let Some(session) = session {
            payload["sessionId"] = json!(session);
        }
        payload.to_string()
    }

    fn overrides_for(&mut self, session: &str) -> Vec<String> {
        let o = self.overrides.clone();
        let mut out = Vec::new();
        let mut push = |method: &str, params: Value| {
            self.next_id += 1;
            let id = self.next_id;
            out.push(
                json!({"id": id, "method": method, "params": params, "sessionId": session})
                    .to_string(),
            );
        };
        push(
            "Emulation.setTimezoneOverride",
            json!({"timezoneId": o.timezone}),
        );
        push(
            "Emulation.setLocaleOverride",
            json!({"locale": o.locale}),
        );
        push(
            "Emulation.setHardwareConcurrencyOverride",
            json!({"hardwareConcurrency": o.hardware_concurrency}),
        );
        push(
            "Emulation.setDeviceMetricsOverride",
            json!({
                "width": o.width,
                "height": o.height,
                "deviceScaleFactor": o.device_scale_factor,
                "mobile": o.mobile,
                "screenWidth": o.screen_width,
                "screenHeight": o.screen_height
            }),
        );
        push(
            "Emulation.setEmulatedMedia",
            json!({
                "features": [{
                    "name": "prefers-color-scheme",
                    "value": if o.color_scheme == "light" { "light" } else { "dark" }
                }]
            }),
        );
        let user_agent = if o.user_agent.is_empty() {
            String::new()
        } else {
            o.user_agent.clone()
        };
        push(
            "Emulation.setUserAgentOverride",
            json!({
                "userAgent": user_agent,
                "acceptLanguage": o.accept_language,
                "platform": o.platform
            }),
        );
        push("Runtime.runIfWaitingForDebugger", json!({}));
        out
    }
}

type Sink = futures_util::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

async fn apply(sink: &mut Sink, state: &mut Session, session_id: &str) {
    for command in state.overrides_for(session_id) {
        if sink.send(Message::Text(command.into())).await.is_err() {
            return;
        }
    }
}

/// Attaches to the browser and keeps applying `overrides` to every page target
/// until the browser exits or `cancel` flips to true.
pub async fn supervise(ws_url: String, overrides: Overrides, mut cancel: watch::Receiver<bool>) {
    use futures_util::{SinkExt, StreamExt};

    let (ws, _) = match tokio_tungstenite::connect_async(ws_url).await {
        Ok(parts) => parts,
        Err(err) => {
            tracing::warn!(error = %err, "could not connect to the browser debug endpoint");
            return;
        }
    };
    let (mut sink, mut stream) = ws.split();
    let mut state = Session {
        next_id: 0,
        pending: HashMap::new(),
        overrides,
    };

    let discover = state.command("Target.setDiscoverTargets", json!({"discover": true}), None, None);
    if sink.send(Message::Text(discover.into())).await.is_err() {
        return;
    }
    let auto = state.command(
        "Target.setAutoAttach",
        json!({"autoAttach": true, "waitForDebuggerOnStart": true, "flatten": true}),
        None,
        None,
    );
    if sink.send(Message::Text(auto.into())).await.is_err() {
        return;
    }
    let list = state.command("Target.getTargets", json!({}), None, Some(Tag::Targets));
    if sink.send(Message::Text(list.into())).await.is_err() {
        return;
    }
    tracing::debug!("CDP supervisor attached");

    loop {
        tokio::select! {
            changed = cancel.changed() => {
                if changed.is_err() || *cancel.borrow() {
                    break;
                }
            }
            incoming = stream.next() => {
                let Some(Ok(message)) = incoming else { break };
                match message {
                    Message::Text(text) => {
                        let Ok(value) = serde_json::from_str::<Value>(&text) else { continue };
                        if let Some(id) = value.get("id").and_then(|v| v.as_u64()) {
                            let tag = state.pending.remove(&id);
                            if let Some(error) = value.get("error") {
                                tracing::debug!(%error, "CDP command rejected");
                                continue;
                            }
                            match tag {
                                Some(Tag::Targets) => {
                                    let targets = value
                                        .get("result")
                                        .and_then(|r| r.get("targetInfos"))
                                        .and_then(|v| v.as_array())
                                        .cloned()
                                        .unwrap_or_default();
                                    for target in targets {
                                        if target.get("type").and_then(|v| v.as_str()) != Some("page") {
                                            continue;
                                        }
                                        let Some(target_id) = target.get("targetId").and_then(|v| v.as_str()) else { continue };
                                        let attach = state.command(
                                            "Target.attachToTarget",
                                            json!({"targetId": target_id, "flatten": true}),
                                            None,
                                            Some(Tag::Attach),
                                        );
                                        if sink.send(Message::Text(attach.into())).await.is_err() {
                                            return;
                                        }
                                    }
                                }
                                Some(Tag::Attach) => {
                                    let session = value
                                        .get("result")
                                        .and_then(|r| r.get("sessionId"))
                                        .and_then(|v| v.as_str())
                                        .map(|v| v.to_string());
                                    if let Some(session) = session {
                                        apply(&mut sink, &mut state, &session).await;
                                    }
                                }
                                None => {}
                            }
                            continue;
                        }
                        if value.get("method").and_then(|v| v.as_str()) == Some("Target.attachedToTarget") {
                            let params = value.get("params").cloned().unwrap_or_default();
                            let is_page = params
                                .get("targetInfo")
                                .and_then(|t| t.get("type"))
                                .and_then(|v| v.as_str())
                                == Some("page");
                            let session = params
                                .get("sessionId")
                                .and_then(|v| v.as_str())
                                .map(|v| v.to_string());
                            if let (true, Some(session)) = (is_page, session.clone()) {
                                apply(&mut sink, &mut state, &session).await;
                            } else if let Some(session) = session {
                                // Nothing to emulate, but the target is paused waiting for us.
                                let resume = state.command(
                                    "Runtime.runIfWaitingForDebugger",
                                    json!({}),
                                    Some(&session),
                                    None,
                                );
                                if sink.send(Message::Text(resume.into())).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                    Message::Ping(payload) => {
                        if sink.send(Message::Pong(payload)).await.is_err() { return }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
        }
    }
    tracing::debug!("CDP supervisor stopped");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overrides() -> Overrides {
        Overrides {
            timezone: "Europe/Berlin".into(),
            locale: "de-DE".into(),
            accept_language: "de-DE,de;q=0.9".into(),
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/131.0.0.0".into(),
            platform: "Win32".into(),
            width: 1920,
            height: 1080,
            screen_width: 1920,
            screen_height: 1080,
            device_scale_factor: 1.0,
            mobile: false,
            color_scheme: "dark".into(),
            hardware_concurrency: 8,
        }
    }

    fn state() -> Session {
        Session {
            next_id: 0,
            pending: HashMap::new(),
            overrides: overrides(),
        }
    }

    #[test]
    fn emits_unique_scoped_override_commands() {
        let mut session = state();
        let commands = session.overrides_for("SESSION1");
        assert_eq!(commands.len(), 7);
        let mut ids = Vec::new();
        for command in &commands {
            let value: Value = serde_json::from_str(command).unwrap();
            assert_eq!(value["sessionId"], "SESSION1");
            ids.push(value["id"].as_u64().unwrap());
        }
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), commands.len(), "command ids must be unique");

        let first: Value = serde_json::from_str(&commands[0]).unwrap();
        assert_eq!(first["method"], "Emulation.setTimezoneOverride");
        assert_eq!(first["params"]["timezoneId"], "Europe/Berlin");
        let last: Value = serde_json::from_str(&commands[commands.len() - 1]).unwrap();
        assert_eq!(last["method"], "Runtime.runIfWaitingForDebugger");
    }

    #[test]
    fn light_scheme_maps_to_light_media_feature() {
        let mut session = Session {
            next_id: 0,
            pending: HashMap::new(),
            overrides: Overrides {
                color_scheme: "light".into(),
                ..overrides()
            },
        };
        let media = session
            .overrides_for("S")
            .iter()
            .map(|c| serde_json::from_str::<Value>(c).unwrap())
            .find(|v| v["method"] == "Emulation.setEmulatedMedia")
            .unwrap();
        assert_eq!(media["params"]["features"][0]["value"], "light");
    }
}
