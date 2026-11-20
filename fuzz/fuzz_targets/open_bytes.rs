//! Cualquier secuencia de bytes abre con error o con una imagen, nunca con
//! un `panic` (RNF-E1) ni pasando de los límites (RNF-S1).
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = pyro_formats::io::open_bytes(data, &pyro_core::task::NoMonitor);
});
