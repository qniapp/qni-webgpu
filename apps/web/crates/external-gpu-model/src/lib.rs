use std::num::NonZeroUsize;
use std::time::Duration;

/// 外部 GPU 実行（Qiskit Aer バックエンド経路）の測定ショット数。
///
/// バックエンドの契約と同じ「1 以上・上限以下」を生成時に強制する値オブジェクト。
/// 量子ビット数や結果側の回数の `usize` と取り違えないよう型で区別する。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Shots(NonZeroUsize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotsError {
    Zero,
    AboveMax { value: usize, max: usize },
}

impl Shots {
    /// 上限。バックエンドの `MAX_SHOTS`
    /// (`apps/qiskit-backend/src/qni_qiskit_backend/contract.py`) と同じ値に保つ。
    /// 片方だけ変えないこと。
    pub const MAX: usize = 100_000;

    /// 既定のショット数。
    pub const DEFAULT: Shots = Shots(match NonZeroUsize::new(1024) {
        Some(value) => value,
        None => unreachable!(),
    });

    pub fn try_new(value: usize) -> Result<Self, ShotsError> {
        let value = NonZeroUsize::new(value).ok_or(ShotsError::Zero)?;
        if value.get() > Self::MAX {
            return Err(ShotsError::AboveMax {
                value: value.get(),
                max: Self::MAX,
            });
        }
        Ok(Self(value))
    }

    pub fn get(self) -> usize {
        self.0.get()
    }
}

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

pub fn qiskit_run_payload(qubits: usize, columns_json: &str, shots: Shots) -> String {
    qiskit_run_payload_with_display_outputs(qubits, columns_json, shots, "[]", "[]", "[]", "[]")
}

pub fn qiskit_run_payload_with_display_outputs(
    qubits: usize,
    columns_json: &str,
    shots: Shots,
    amplitudes_json: &str,
    bloch_json: &str,
    probability_json: &str,
    densities_json: &str,
) -> String {
    let shots = shots.get();
    let amplitudes = optional_output_json("amplitudes", amplitudes_json);
    let bloch = optional_output_json("bloch", bloch_json);
    let probability = optional_output_json("probability", probability_json);
    let densities = optional_output_json("densities", densities_json);
    format!(
        r#"{{"qubits":{qubits},"columns":{columns_json},"shots":{shots},"outputs":{{"histogram":true{amplitudes}{bloch}{probability}{densities}}}}}"#
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
        other if other.starts_with("Density") => "Density Matrix".to_owned(),
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
        unsupported_gate_from_message, Shots, ShotsError,
    };

    #[test]
    fn shots_keeps_valid_value() {
        assert_eq!(Shots::try_new(256).map(Shots::get), Ok(256));
    }

    #[test]
    fn shots_rejects_zero() {
        assert_eq!(Shots::try_new(0), Err(ShotsError::Zero));
    }

    #[test]
    fn shots_rejects_above_max() {
        assert_eq!(
            Shots::try_new(Shots::MAX + 1),
            Err(ShotsError::AboveMax {
                value: 100_001,
                max: 100_000,
            })
        );
    }

    #[test]
    fn shots_accepts_max() {
        assert_eq!(Shots::try_new(Shots::MAX).map(Shots::get), Ok(100_000));
    }

    #[test]
    fn shots_default_is_1024() {
        assert_eq!(Shots::DEFAULT.get(), 1024);
    }

    #[test]
    fn equal_shots_compare_by_value() {
        assert_eq!(Shots::try_new(512).unwrap(), Shots::try_new(512).unwrap());
    }

    #[test]
    fn payload_embeds_default_shots() {
        assert!(qiskit_run_payload(2, r#"[["H",1]]"#, Shots::DEFAULT).contains(r#""shots":1024"#));
    }

    #[test]
    fn payload_requests_histogram() {
        let payload = qiskit_run_payload(2, r#"[["H",1]]"#, Shots::try_new(256).unwrap());
        assert_eq!(
            payload.as_str(),
            r#"{"qubits":2,"columns":[["H",1]],"shots":256,"outputs":{"histogram":true}}"#,
        );
    }

    #[test]
    fn payload_omits_full_vector_outputs() {
        let payload = qiskit_run_payload(2, r#"[["H",1]]"#, Shots::try_new(256).unwrap());
        assert!(!(payload.contains("statevector") || payload.contains("probabilities")));
    }

    #[test]
    fn payload_can_request_amplitude_display_outputs() {
        let payload = qiskit_run_payload_with_display_outputs(
            1,
            r#"[["H"],["Amps1"]]"#,
            Shots::try_new(256).unwrap(),
            r#"[{"gate_id":2,"column":1,"span":1,"base_bit":0,"control_mask":0,"control_value":0,"phase_lock_enabled":false}]"#,
            "[]",
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
            Shots::try_new(256).unwrap(),
            "[]",
            r#"[{"gate_id":2,"column":1,"wire":0}]"#,
            "[]",
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
            Shots::try_new(256).unwrap(),
            "[]",
            "[]",
            r#"[{"gate_id":2,"column":1,"span":1,"base_bit":0}]"#,
            "[]",
        );
        assert_eq!(
            payload.as_str(),
            r#"{"qubits":1,"columns":[["H"],["Probability"]],"shots":256,"outputs":{"histogram":true,"probability":[{"gate_id":2,"column":1,"span":1,"base_bit":0}]}}"#,
        );
    }

    #[test]
    fn payload_can_request_density_display_outputs() {
        let payload = qiskit_run_payload_with_display_outputs(
            1,
            r#"[["H"],["Density"]]"#,
            Shots::try_new(256).unwrap(),
            "[]",
            "[]",
            "[]",
            r#"[{"gate_id":2,"column":1,"span":1,"base_bit":0}]"#,
        );
        assert_eq!(
            payload.as_str(),
            r#"{"qubits":1,"columns":[["H"],["Density"]],"shots":256,"outputs":{"histogram":true,"densities":[{"gate_id":2,"column":1,"span":1,"base_bit":0}]}}"#,
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

    #[test]
    fn unsupported_gate_token_message_maps_density() {
        assert_eq!(
            unsupported_gate_from_message("unsupported gate token: Density2"),
            Some("Density Matrix".to_owned()),
        );
    }
}
