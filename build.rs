use std::env;

fn main() {
    let build_enabled = env::var("BINDGEN_ENABLED")
        .map(|v| v == "1")
        .unwrap_or(true); // run by default

    if build_enabled {
        node_bindgen::build::configure();
    }
}
