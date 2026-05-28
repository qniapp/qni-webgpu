use qni_web_circuit_library_model::{CircuitId, CircuitOrigin};

fn main() {
    let _ = CircuitOrigin::Sample {
        origin_id: CircuitId::try_new("bell").unwrap(),
        locked: false,
    };
}
