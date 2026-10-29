use wattlab_rust_smoke::average_power_w;

fn main() {
    let samples = [Some(100), None, Some(0)];
    match average_power_w(&samples) {
        Some(watts) => println!("Rust toolchain ready. Example average power: {watts:.1} W"),
        None => println!("No power measurements received."),
    }
}
