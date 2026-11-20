//! Muestra los bloques PNG de un archivo y si es de Fireworks.
//! Uso: cargo run --example fwinfo -- archivo.png

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("uso: fwinfo <archivo.png>");
    let opened = pyro_formats::io::open_path(path.as_ref(), &pyro_core::task::NoMonitor)?;
    let doc = &opened.document;
    println!("Imagen: {}×{} px, capa \"{}\"", doc.width, doc.height, doc.layers[0].name);
    if let Some(info) = opened.png_info {
        println!("Fireworks: {}", if info.is_fireworks() { "sí" } else { "no" });
        for c in &info.chunks {
            println!("  {:4}  {:>9} bytes{}", c.kind, c.length, if c.is_private() { "  (privado)" } else { "" });
        }
    }
    Ok(())
}
