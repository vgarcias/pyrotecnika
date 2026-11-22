//! Un `.ora` cualquiera (zip, stack.xml y capas) abre con error o con un
//! documento, nunca con un `panic` ni pasando de los límites.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = pyro_formats::ora::decode_ora(data, &pyro_core::task::NoMonitor);
});
