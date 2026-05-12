//! Browser URL writer for the current circuit hash.

/// Push the serialised circuit into `location.hash`, Quirk-style but
/// without the `circuit=` key prefix:
///   #{"cols":[...]}
///
/// JSON is written raw; falls back to `encodeURIComponent` only when
/// the payload contains `%` (which would otherwise be interpreted as
/// the start of a percent-escape sequence on read-back). `&` is no
/// longer a hazard since we don't split the hash into key=value pairs.
/// Uses `history.replaceState` so navigation back/forward doesn't fill
/// up with one entry per drop.
#[cfg(target_arch = "wasm32")]
pub(crate) fn write_circuit_to_url(json: &str) {
    use wasm_bindgen::JsValue;
    let Some(window) = web_sys::window() else {
        return;
    };
    let payload = if json.contains('%') {
        js_sys::encode_uri_component(json)
            .as_string()
            .unwrap_or_else(|| json.to_string())
    } else {
        json.to_string()
    };
    let hash = format!("#{payload}");
    // Compose a full URL so `replaceState` doesn't rewrite the path.
    let location = window.location();
    let base = match (location.origin(), location.pathname(), location.search()) {
        (Ok(o), Ok(p), Ok(s)) => format!("{o}{p}{s}"),
        _ => return,
    };
    let url = format!("{base}{hash}");
    let _ = window.history().ok().and_then(|h| {
        h.replace_state_with_url(&JsValue::NULL, "", Some(&url))
            .ok()
    });
}

/// Non-wasm stub so callers don't need to `cfg`-gate.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn write_circuit_to_url(_json: &str) {}
