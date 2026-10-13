//! Gegenstelle ohne Oberfläche (zum Testen): cargo run --example gegenstelle -- 4711
#[tokio::main]
async fn main() {
    let code = std::env::args().nth(1).unwrap_or_else(speedtest_lib::server::new_code);
    let _h = speedtest_lib::server::start(speedtest_lib::net::PORT, code.clone(), std::sync::Arc::new(|e| eprintln!("{e:?}"))).await.unwrap();
    println!("Gegenstelle bereit, Code {code}");
    tokio::signal::ctrl_c().await.ok();
}
