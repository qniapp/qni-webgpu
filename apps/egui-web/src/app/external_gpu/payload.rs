pub(super) fn qiskit_run_payload(qubits: usize, columns_json: &str, shots: usize) -> String {
    format!(
        r#"{{"qubits":{qubits},"columns":{columns_json},"shots":{shots},"outputs":{{"histogram":true}}}}"#
    )
}

#[cfg(test)]
mod tests {
    use super::qiskit_run_payload;

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
}
