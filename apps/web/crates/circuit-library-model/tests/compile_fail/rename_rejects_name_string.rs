use qni_web_circuit_library_model::CircuitLibrary;

fn main() {
    let mut library = CircuitLibrary::seed();
    library.rename(&"Bell state".to_owned(), "Renamed");
}
