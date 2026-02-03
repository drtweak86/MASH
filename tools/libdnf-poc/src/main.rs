extern "C" {
    fn libdnf5_probe_touch();
}

fn main() {
    unsafe {
        libdnf5_probe_touch();
    }
    println!("libdnf5 probe OK");
}
