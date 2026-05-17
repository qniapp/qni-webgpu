use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};

const ICONS: [(&str, &str); 5] = [
    ("H", "h.png"),
    ("X", "x.png"),
    ("Y", "y.png"),
    ("Z", "z.png"),
    ("PLUS", "plus.png"),
];
const RASTER_SIZE: u32 = 128;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("out dir"));
    let out_path = out_dir.join("gate_icon_alpha.rs");
    let mut generated = String::new();
    generated.push_str("pub(super) const RASTER_SIZE: usize = 128;\n");

    for (symbol, file_name) in ICONS {
        let path = manifest_dir.join("assets/icons").join(file_name);
        println!("cargo:rerun-if-changed={}", path.display());
        let alpha = read_png_alpha(&path);
        let rle = encode_rle(&alpha);
        generated.push_str(&format!(
            "pub(super) const {symbol}_ALPHA_RLE: &[(u16, u8)] = &["
        ));
        for (run, value) in rle {
            generated.push_str(&format!("({run},{value}),"));
        }
        generated.push_str("];\n");
    }

    fs::write(out_path, generated).expect("write generated icon alpha data");
}

fn read_png_alpha(path: &Path) -> Vec<u8> {
    let file = fs::File::open(path).unwrap_or_else(|error| {
        panic!("failed to open {}: {error}", path.display());
    });
    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder.read_info().unwrap_or_else(|error| {
        panic!("failed to read {}: {error}", path.display());
    });
    let mut buffer = vec![0; reader.output_buffer_size().expect("png output size")];
    let info = reader.next_frame(&mut buffer).unwrap_or_else(|error| {
        panic!("failed to decode {}: {error}", path.display());
    });
    assert_eq!(
        (info.width, info.height, info.color_type, info.bit_depth),
        (
            RASTER_SIZE,
            RASTER_SIZE,
            png::ColorType::Rgba,
            png::BitDepth::Eight
        ),
        "{} must be a 128×128 8-bit RGBA PNG",
        path.display()
    );
    let alpha = buffer[..info.buffer_size()]
        .chunks_exact(4)
        .map(|rgba| rgba[3])
        .collect::<Vec<_>>();
    assert!(
        alpha.iter().any(|value| *value > 0),
        "{} must contain visible glyph pixels",
        path.display()
    );
    alpha
}

fn encode_rle(alpha: &[u8]) -> Vec<(u16, u8)> {
    let mut encoded = Vec::new();
    let mut iter = alpha.iter().copied();
    let Some(mut value) = iter.next() else {
        return encoded;
    };
    let mut run: u16 = 1;
    for next in iter {
        if next == value && run < u16::MAX {
            run += 1;
        } else {
            encoded.push((run, value));
            value = next;
            run = 1;
        }
    }
    encoded.push((run, value));
    encoded
}
