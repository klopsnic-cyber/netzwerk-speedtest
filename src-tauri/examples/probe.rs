fn main() {
    for i in speedtest_lib::iface::list() {
        println!("{i:?}");
    }
    println!("VPN: {:?}  Akku: {}", speedtest_lib::iface::active_vpns(), speedtest_lib::iface::on_battery());
}
