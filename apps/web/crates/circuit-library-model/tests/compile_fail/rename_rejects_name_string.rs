use qni_web_circuit_library_model::CircuitLibrary;

fn main() {
    let mut library = CircuitLibrary::seed();
    // `rename` takes `&CircuitId`; a plain name `String` must not be accepted.
    library.rename(&"Bell state".to_owned(), "Renamed");
}
