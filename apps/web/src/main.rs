#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "qni-web",
        options,
        Box::new(|cc| Box::new(qni_web::QniApp::new(cc))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
