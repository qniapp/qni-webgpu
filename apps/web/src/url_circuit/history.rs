//! Browser URL writer for the current circuit hash and execution mode query.

use crate::app::ExecMode;

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

#[cfg(target_arch = "wasm32")]
pub(crate) fn parse_exec_mode_from_url() -> Option<ExecMode> {
    let window = web_sys::window()?;
    let search = window.location().search().ok()?;
    exec_mode_from_search(&search)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn parse_exec_mode_from_url() -> Option<ExecMode> {
    None
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn write_exec_mode_to_url(mode: ExecMode) {
    use wasm_bindgen::JsValue;
    let Some(window) = web_sys::window() else {
        return;
    };
    let location = window.location();
    let search = location.search().unwrap_or_default();
    let next_search = search_with_exec_mode(&search, mode);
    let base = match (location.origin(), location.pathname()) {
        (Ok(o), Ok(p)) => format!("{o}{p}"),
        _ => return,
    };
    let url = format!("{base}{next_search}{}", location.hash().unwrap_or_default());
    let _ = window.history().ok().and_then(|h| {
        h.replace_state_with_url(&JsValue::NULL, "", Some(&url))
            .ok()
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn write_exec_mode_to_url(_mode: ExecMode) {}

fn exec_mode_from_search(search: &str) -> Option<ExecMode> {
    let body = search.strip_prefix('?').unwrap_or(search);
    let mut mode = None;
    for part in body.split('&').filter(|part| !part.is_empty()) {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        if key != "mode" {
            continue;
        }
        mode = if value.eq_ignore_ascii_case("gpu") {
            Some(ExecMode::Gpu)
        } else if value.eq_ignore_ascii_case("local") {
            Some(ExecMode::Local)
        } else {
            mode
        };
    }
    mode
}

fn search_with_exec_mode(search: &str, mode: ExecMode) -> String {
    let body = search.strip_prefix('?').unwrap_or(search);
    let mut parts: Vec<String> = body
        .split('&')
        .filter(|part| !part.is_empty())
        .filter(|part| {
            part.split_once('=')
                .map_or(*part != "mode", |(key, _)| key != "mode")
        })
        .map(str::to_string)
        .collect();
    if mode == ExecMode::Gpu {
        parts.push("mode=gpu".to_string());
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    }
}

/// Non-wasm stub so callers don't need to `cfg`-gate.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn write_circuit_to_url(_json: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_mode_from_search_reads_gpu_mode() {
        assert_eq!(
            exec_mode_from_search("?foo=1&mode=gpu"),
            Some(ExecMode::Gpu)
        );
    }

    #[test]
    fn exec_mode_from_search_reads_local_mode() {
        assert_eq!(
            exec_mode_from_search("?mode=local&foo=1"),
            Some(ExecMode::Local)
        );
    }

    #[test]
    fn search_with_exec_mode_writes_gpu_mode() {
        assert_eq!(
            search_with_exec_mode("?foo=1", ExecMode::Gpu),
            "?foo=1&mode=gpu"
        );
    }

    #[test]
    fn search_with_exec_mode_removes_local_mode() {
        assert_eq!(
            search_with_exec_mode("?foo=1&mode=gpu", ExecMode::Local),
            "?foo=1"
        );
    }
}
