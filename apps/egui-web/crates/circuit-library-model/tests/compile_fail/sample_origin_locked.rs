use qni_egui_web_circuit_library_model::CircuitOrigin;

fn main() {
    let _ = CircuitOrigin::Sample {
        origin_id: "bell".to_owned(),
        locked: false,
    };
}
