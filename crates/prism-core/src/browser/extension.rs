//! Per-profile browser extension.
//!
//! Chromium supports content scripts that run in the page's own JavaScript
//! world before any site script. Prism uses that documented mechanism for the
//! privacy controls that are not exposed on the command line: deterministic
//! canvas/WebGL readback noise, `navigator` hints, and the WebRTC policy.
//!
//! The extension is generated per launch into the profile directory so each
//! profile gets its own seed and policy, and it is removed from
//! `--disable-extensions-except` when every control is off.

use crate::error::Result;
use crate::model::Fingerprint;
use serde_json::json;
use std::path::Path;

pub struct ExtensionPlan {
    pub dir: std::path::PathBuf,
    pub enabled: bool,
}

/// Writes (or refreshes) the extension for a profile.
pub fn prepare(dir: &Path, fingerprint: &Fingerprint) -> Result<ExtensionPlan> {
    if std::fs::create_dir_all(dir).is_err() {
        return Ok(ExtensionPlan {
            dir: dir.to_path_buf(),
            enabled: false,
        });
    }

    let uses_webrtc_policy = fingerprint.webrtc != "default";
    let enabled = fingerprint.canvas_noise
        || fingerprint.webgl_noise
        || uses_webrtc_policy
        || fingerprint.device_memory_gb > 0
        || fingerprint.hardware_concurrency > 0;

    if !enabled {
        return Ok(ExtensionPlan {
            dir: dir.to_path_buf(),
            enabled: false,
        });
    }

    let manifest = json!({
        "manifest_version": 3,
        "name": "Prism Profile Guard",
        "version": "1.0.0",
        "description": "Per-profile privacy hardening applied by Prism.",
        "permissions": ["privacy"],
        "minimum_chrome_version": "111",
        "background": { "service_worker": "background.js", "type": "module" },
        "content_scripts": [{
            "matches": ["http://*/*", "https://*/*"],
            "js": ["guard.js"],
            "run_at": "document_start",
            "all_frames": true,
            "world": "MAIN"
        }]
    });
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;

    let config = format!(
        "export default {};\n",
        serde_json::to_string(&json!({
            "webrtc": fingerprint.webrtc,
            "seed": fingerprint.seed,
            "canvasNoise": fingerprint.canvas_noise,
            "webglNoise": fingerprint.webgl_noise,
            "hardwareConcurrency": fingerprint.hardware_concurrency,
            "deviceMemoryGb": fingerprint.device_memory_gb,
            "platform": fingerprint.platform,
        }))?
    );
    std::fs::write(dir.join("config.js"), config)?;

    let background = r#"import cfg from './config.js';

// Chromium's documented WebRTC privacy control: when enabled, ICE candidates
// that would expose the local network are replaced by the proxy's own address.
const policy = cfg.webrtc === 'default' ? 'default' : 'disable_non_proxied_udp';
try {
  chrome.privacy.network.webRTCIPHandlingPolicy.set({ value: policy, scope: 'regular' });
} catch (error) {
  console.warn('[Prism] WebRTC policy could not be applied:', error);
}
"#;
    std::fs::write(dir.join("background.js"), background)?;

    std::fs::write(dir.join("guard.js"), guard_script(fingerprint))?;
    Ok(ExtensionPlan {
        dir: dir.to_path_buf(),
        enabled: true,
    })
}

fn guard_script(fingerprint: &Fingerprint) -> String {
    format!(
        r#"(() => {{
  'use strict';
  const CFG = {cfg};
  // Deterministic xorshift PRNG: the same profile always reports the same
  // values, so a site sees a stable device instead of a random one.
  let state = (CFG.seed >>> 0) || 1;
  const rand = () => {{
    state ^= state << 13; state >>>= 0;
    state ^= state >>> 17;
    state ^= state << 5;  state >>>= 0;
    return state / 4294967296;
  }};

  const tweak = (data) => {{
    if (!data || !data.length) return;
    const pixels = Math.min(24, Math.floor(data.length / 4));
    for (let i = 0; i < pixels; i++) {{
      const offset = Math.floor(rand() * (data.length - 4) / 4) * 4;
      data[offset] = (data[offset] + (rand() < 0.5 ? 1 : 255)) & 0xff;
    }}
  }};

  const patchCanvas = () => {{
    const proto = HTMLCanvasElement.prototype;
    for (const method of ['toDataURL', 'toBlob']) {{
      const original = proto[method];
      if (typeof original !== 'function') continue;
      proto[method] = function (...args) {{
        try {{
          const ctx = this.getContext('2d');
          if (ctx) {{
            const frame = ctx.getImageData(0, 0, this.width, this.height);
            tweak(frame.data);
            ctx.putImageData(frame, 0, 0);
          }}
        }} catch (error) {{ /* tainted or WebGL canvas */ }}
        return original.apply(this, args);
      }};
    }}
    const ctxProto = CanvasRenderingContext2D.prototype;
    const getImageData = ctxProto.getImageData;
    if (typeof getImageData === 'function') {{
      ctxProto.getImageData = function (...args) {{
        const frame = getImageData.apply(this, args);
        try {{ tweak(frame.data); }} catch (error) {{}}
        return frame;
      }};
    }}
  }};

  const patchWebgl = () => {{
    for (const name of ['WebGLRenderingContext', 'WebGL2RenderingContext']) {{
      const ctor = window[name];
      if (!ctor) continue;
      const readPixels = ctor.prototype.readPixels;
      if (typeof readPixels === 'function') {{
        ctor.prototype.readPixels = function (...args) {{
          const result = readPixels.apply(this, args);
          try {{
            const pixels = args[6];
            if (pixels && pixels.length) tweak(new Uint8Array(pixels.buffer || pixels));
          }} catch (error) {{}}
          return result;
        }};
      }}
      const getParameter = ctor.prototype.getParameter;
      if (typeof getParameter === 'function') {{
        ctor.prototype.getParameter = function (parameter) {{
          // UNMASKED_VENDOR_WEBGL / UNMASKED_RENDERER_WEBGL
          if (parameter === 0x9245) return 'Google Inc. (Intel)';
          if (parameter === 0x9246) {{
            return 'ANGLE (Intel, Intel(R) UHD Graphics 630 Direct3D11 vs_5_0 ps_5_0, D3D11)';
          }}
          return getParameter.apply(this, arguments);
        }};
      }}
    }}
  }};

  const patchNavigator = () => {{
    const define = (key, value) => {{
      if (typeof value !== 'number' || value <= 0) return;
      try {{
        Object.defineProperty(Navigator.prototype, key, {{
          get: () => value,
          configurable: true
        }});
      }} catch (error) {{}}
    }};
    define('hardwareConcurrency', CFG.hardwareConcurrency);
    define('deviceMemory', CFG.deviceMemoryGb);
  }};

  if (CFG.webrtc === 'disabled') {{
    try {{
      const blocked = function () {{ throw new Error('WebRTC is disabled by this profile'); }};
      Object.defineProperty(window, 'RTCPeerConnection', {{ value: blocked, configurable: true }});
      Object.defineProperty(window, 'webkitRTCPeerConnection', {{ value: blocked, configurable: true }});
    }} catch (error) {{}}
  }}

  if (CFG.canvasNoise) {{ patchCanvas(); }}
  if (CFG.webglNoise) {{ patchWebgl(); }}
  patchNavigator();
}})();
"#,
        cfg = serde_json::to_string(&json!({
            "webrtc": fingerprint.webrtc,
            "seed": fingerprint.seed,
            "canvasNoise": fingerprint.canvas_noise,
            "webglNoise": fingerprint.webgl_noise,
            "hardwareConcurrency": fingerprint.hardware_concurrency,
            "deviceMemoryGb": fingerprint.device_memory_gb,
            "platform": fingerprint.platform,
        }))
        .unwrap_or_else(|_| "{}".into())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_extension_files_with_seed() {
        let dir = tempfile::tempdir().unwrap();
        let mut fingerprint = Fingerprint::default();
        fingerprint.seed = 42;
        let plan = prepare(dir.path(), &fingerprint).unwrap();
        assert!(plan.enabled);
        assert!(dir.path().join("manifest.json").exists());
        let guard = std::fs::read_to_string(dir.path().join("guard.js")).unwrap();
        assert!(guard.contains("\"seed\":42"));
        assert!(guard.contains("hardwareConcurrency"));
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.path().join("manifest.json")).unwrap())
                .unwrap();
        assert_eq!(manifest["manifest_version"], 3);
    }

    #[test]
    fn disables_itself_when_every_control_is_off() {
        let dir = tempfile::tempdir().unwrap();
        let fingerprint = Fingerprint {
            webrtc: "default".into(),
            canvas_noise: false,
            webgl_noise: false,
            hardware_concurrency: 0,
            device_memory_gb: 0,
            ..Fingerprint::default()
        };
        let plan = prepare(dir.path(), &fingerprint).unwrap();
        assert!(!plan.enabled);
    }
}
