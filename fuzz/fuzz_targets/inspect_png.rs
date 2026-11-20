//! El lector de bloques de Fireworks no cae con bloques de longitud o CRC
//! imposibles.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = pyro_formats::fwpng::inspect_png(data);
});
