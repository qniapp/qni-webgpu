use std::time::Duration;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum ExternalGpuStatus {
    #[default]
    Idle,
    Running,
    Completed {
        duration: Duration,
    },
    Failed(GpuFailure),
}

impl ExternalGpuStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GpuFailure {
    BackendOffline { endpoint: String },
    UnsupportedGate(String),
    Http(u16),
    Other(String),
}

impl GpuFailure {
    pub fn label(&self) -> &'static str {
        match self {
            Self::BackendOffline { .. } => "Backend unreachable",
            Self::UnsupportedGate(_) => "Unsupported gate",
            Self::Http(_) => "Backend error",
            Self::Other(_) => "GPU failed",
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Self::BackendOffline { endpoint } => endpoint.clone(),
            Self::UnsupportedGate(name) => name.clone(),
            Self::Http(status) => format!("HTTP {status}"),
            Self::Other(label) => label.clone(),
        }
    }
}

pub fn qiskit_run_payload(qubits: usize, columns_json: &str, shots: usize) -> String {
    qiskit_run_payload_with_display_outputs(qubits, columns_json, shots, "[]", "[]", "[]")
}

pub fn qiskit_run_payload_with_display_outputs(
    qubits: usize,
    columns_json: &str,
    shots: usize,
    amplitudes_json: &str,
    bloch_json: &str,
    probability_json: &str,
) -> String {
    let amplitudes = optional_output_json("amplitudes", amplitudes_json);
    let bloch = optional_output_json("bloch", bloch_json);
    let probability = optional_output_json("probability", probability_json);
    format!(
        r#"{{"qubits":{qubits},"columns":{columns_json},"shots":{shots},"outputs":{{"histogram":true{amplitudes}{bloch}{probability}}}}}"#
    )
}

fn optional_output_json(name: &str, value_json: &str) -> String {
    if value_json.trim() == "[]" {
        String::new()
    } else {
        format!(r#","{name}":{value_json}"#)
    }
}

pub fn format_gpu_duration(duration: Duration) -> String {
    let millis = duration.as_millis();
    if millis < 1_000 {
        format!("{millis} ms")
    } else {
        format!("{:.1} s", duration.as_secs_f32())
    }
}

pub fn unsupported_gate_from_message(message: &str) -> Option<String> {
    let lower = message.to_ascii_lowercase();
    if let Some((_, token)) = message.split_once("unsupported gate token:") {
        return Some(normalize_gate_name(token.trim()));
    }
    if lower.contains("anti-control") {
        return Some("Anti-control".to_owned());
    }
    if lower.contains("qft") {
        return Some("QFT".to_owned());
    }
    if lower.contains("controlled non-x") {
        return Some("controlled gate".to_owned());
    }
    None
}

fn normalize_gate_name(token: &str) -> String {
    match token {
        "…" => "Spacer".to_owned(),
        "Measure" => "Measurement".to_owned(),
        "Bloch" => "Bloch".to_owned(),
        other if other.starts_with("Amps") => "Amplitude".to_owned(),
        other if other.starts_with("Probability") => "Probability".to_owned(),
        other => other.to_owned(),
    }
}

pub fn short_failure_label(message: &str) -> String {
    const MAX_CHARS: usize = 32;
    let trimmed = message.trim();
    if trimmed.chars().count() <= MAX_CHARS {
        return trimmed.to_owned();
    }
    let mut label: String = trimmed.chars().take(MAX_CHARS - 1).collect();
    label.push('…');
    label
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        format_gpu_duration, qiskit_run_payload, qiskit_run_payload_with_display_outputs,
        unsupported_gate_from_message,
    };

    #[test]
    fn payload_requests_histogram() {
        let payload = qiskit_run_payload(2, r#"[["H",1]]"#, 256);
        assert_eq!(
            payload.as_str(),
            r#"{"qubits":2,"columns":[["H",1]],"shots":256,"outputs":{"histogram":true}}"#,
        );
    }

    #[test]
    fn payload_omits_full_vector_outputs() {
        let payload = qiskit_run_payload(2, r#"[["H",1]]"#, 256);
        assert!(!(payload.contains("statevector") || payload.contains("probabilities")));
    }

    #[test]
    fn payload_can_request_amplitude_display_outputs() {
        let payload = qiskit_run_payload_with_display_outputs(
            1,
            r#"[["H"],["Amps1"]]"#,
            256,
            r#"[{"gate_id":2,"column":1,"span":1,"base_bit":0,"control_mask":0,"control_value":0,"phase_lock_enabled":false}]"#,
            "[]",
            "[]",
        );
        assert_eq!(
            payload.as_str(),
            r#"{"qubits":1,"columns":[["H"],["Amps1"]],"shots":256,"outputs":{"histogram":true,"amplitudes":[{"gate_id":2,"column":1,"span":1,"base_bit":0,"control_mask":0,"control_value":0,"phase_lock_enabled":false}]}}"#,
        );
    }

    #[test]
    fn payload_can_request_bloch_display_outputs() {
        let payload = qiskit_run_payload_with_display_outputs(
            1,
            r#"[["H"],["Bloch"]]"#,
            256,
            "[]",
            r#"[{"gate_id":2,"column":1,"wire":0}]"#,
            "[]",
        );
        assert_eq!(
            payload.as_str(),
            r#"{"qubits":1,"columns":[["H"],["Bloch"]],"shots":256,"outputs":{"histogram":true,"bloch":[{"gate_id":2,"column":1,"wire":0}]}}"#,
        );
    }

    #[test]
    fn payload_can_request_probability_display_outputs() {
        let payload = qiskit_run_payload_with_display_outputs(
            1,
            r#"[["H"],["Probability"]]"#,
            256,
            "[]",
            "[]",
            r#"[{"gate_id":2,"column":1,"span":1,"base_bit":0}]"#,
        );
        assert_eq!(
            payload.as_str(),
            r#"{"qubits":1,"columns":[["H"],["Probability"]],"shots":256,"outputs":{"histogram":true,"probability":[{"gate_id":2,"column":1,"span":1,"base_bit":0}]}}"#,
        );
    }

    #[test]
    fn duration_uses_milliseconds_under_one_second() {
        assert_eq!(format_gpu_duration(Duration::from_millis(420)), "420 ms");
    }

    #[test]
    fn duration_uses_seconds_at_one_second_or_more() {
        assert_eq!(format_gpu_duration(Duration::from_millis(1_400)), "1.4 s");
    }

    #[test]
    fn unsupported_gate_token_message_maps_spacer() {
        assert_eq!(
            unsupported_gate_from_message("unsupported gate token: …"),
            Some("Spacer".to_owned()),
        );
    }

    #[test]
    fn unsupported_gate_text_message_maps_qft() {
        assert_eq!(
            unsupported_gate_from_message(
                "QFT tokens are not supported by the dev Qiskit runner yet"
            ),
            Some("QFT".to_owned()),
        );
    }

    #[test]
    fn unsupported_gate_token_message_maps_amplitude() {
        assert_eq!(
            unsupported_gate_from_message("unsupported gate token: Amps1"),
            Some("Amplitude".to_owned()),
        );
    }

    #[test]
    fn unsupported_gate_token_message_maps_probability() {
        assert_eq!(
            unsupported_gate_from_message("unsupported gate token: Probability2"),
            Some("Probability".to_owned()),
        );
    }
}
