/// Stable codes shared with `qni-embed-error.mjs`.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StartupErrorCode {
    GpuAdapterUnavailable,
    GpuDeviceUnavailable,
    InvalidEmbed,
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl StartupErrorCode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::GpuAdapterUnavailable => "gpu-adapter-unavailable",
            Self::GpuDeviceUnavailable => "gpu-device-unavailable",
            Self::InvalidEmbed => "invalid-embed",
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn startup_error(code: StartupErrorCode, message: &str) -> wasm_bindgen::JsValue {
    let error = js_sys::Error::new(message);
    let _ = js_sys::Reflect::set(&error, &"code".into(), &code.as_str().into());
    error.into()
}

#[cfg(test)]
mod tests {
    #[test]
    fn codes_match_the_embed_error_module() {
        let module = include_str!("../qni-embed-error.mjs");
        let codes = [
            super::StartupErrorCode::GpuAdapterUnavailable,
            super::StartupErrorCode::GpuDeviceUnavailable,
            super::StartupErrorCode::InvalidEmbed,
        ];
        assert!(codes
            .iter()
            .all(|code| module.contains(&format!("'{}'", code.as_str()))));
    }
}
