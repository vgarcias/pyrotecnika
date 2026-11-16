//! Incrusta el ícono y los metadatos de versión en el .exe de Windows. En
//! macOS y Linux el ícono lo ponen los paquetes (.app, .desktop).

fn main() {
    println!("cargo:rerun-if-changed=../../packaging/icons/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../packaging/icons/icon.ico")
            .set("ProductName", "Pyrotecnika")
            .set("FileDescription", "Pyrotecnika");
        res.compile().expect("no se pudo incrustar el ícono en el ejecutable");
    }
}
