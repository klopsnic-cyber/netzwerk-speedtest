//! Gegenstelle („Server-Modus“): wartet auf TCP/UDP-Port 5299, meldet sich per Bonjour/mDNS,
//! akzeptiert nur Clients mit dem angezeigten 4-stelligen Code.

use crate::net::{self, Hello, Reply};
use serde::Serialize;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::task::JoinHandle;

#[derive(Debug, Clone, Serialize)]
pub struct ServerEvent {
    pub peer: String,
    /// "verbunden" | "download" | "upload" | "abgewiesen"
    pub kind: String,
}

pub type EventFn = Arc<dyn Fn(ServerEvent) + Send + Sync>;

pub struct ServerHandle {
    tasks: Vec<JoinHandle<()>>,
    mdns: Option<mdns_sd::ServiceDaemon>,
    pub code: String,
    pub port: u16,
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        for t in &self.tasks {
            t.abort();
        }
        if let Some(d) = self.mdns.take() {
            let _ = d.shutdown();
        }
    }
}

pub fn new_code() -> String {
    format!("{:04}", rand::random_range(0..10_000u32))
}

pub fn host_name() -> String {
    sysinfo::System::host_name().unwrap_or_else(|| "Gegenstelle".into())
}

pub async fn start(port: u16, code: String, on_event: EventFn) -> io::Result<ServerHandle> {
    let tcp = TcpListener::bind(("0.0.0.0", port)).await.map_err(|e| {
        io::Error::new(e.kind(), format!("Port {port} ist belegt oder gesperrt ({e}). Läuft die Gegenstelle schon?"))
    })?;
    let udp = UdpSocket::bind(("0.0.0.0", port)).await?;
    let name = host_name();

    let mut tasks = Vec::new();
    {
        let (code, name, on_event) = (code.clone(), name.clone(), on_event.clone());
        tasks.push(tokio::spawn(async move {
            loop {
                let Ok((s, peer)) = tcp.accept().await else { continue };
                let (code, name, on_event) = (code.clone(), name.clone(), on_event.clone());
                tokio::spawn(async move {
                    let _ = handle(s, peer, &code, &name, &on_event).await;
                });
            }
        }));
    }
    {
        let code = code.clone();
        tasks.push(tokio::spawn(async move {
            let mut buf = [0u8; 1500];
            loop {
                let Ok((n, peer)) = udp.recv_from(&mut buf).await else { continue };
                if n >= 12 && &buf[..4] == net::UDP_MAGIC && &buf[4..8] == code.as_bytes() {
                    let _ = udp.send_to(&buf[..n], peer).await;
                }
            }
        }));
    }

    Ok(ServerHandle { tasks, mdns: announce(&name, port).ok(), code, port })
}

/// Bonjour-Anmeldung, damit der Client die Gegenstelle ohne IP-Eingabe findet (best effort).
fn announce(name: &str, port: u16) -> Result<mdns_sd::ServiceDaemon, mdns_sd::Error> {
    let d = mdns_sd::ServiceDaemon::new()?;
    let label: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let info = mdns_sd::ServiceInfo::new(
        net::MDNS_TYPE,
        name,
        &format!("{label}.local."),
        "",
        port,
        &[("v", net::VERSION.to_string().as_str())][..],
    )?
    .enable_addr_auto();
    d.register(info)?;
    Ok(d)
}

async fn handle(mut s: TcpStream, peer: SocketAddr, code: &str, name: &str, ev: &EventFn) -> io::Result<()> {
    let _ = s.set_nodelay(true);
    let _ = socket2::SockRef::from(&s).set_send_buffer_size(net::SOCK_BUF);
    let _ = socket2::SockRef::from(&s).set_recv_buffer_size(net::SOCK_BUF);
    let hello: Hello = net::read_hello(&mut s).await?;
    let peer_s = peer.ip().to_string();
    let deny = |msg: &str| Reply { ok: false, name: String::new(), error: msg.into() };
    let reply = if hello.v != net::VERSION {
        deny("Andere Programmversion auf beiden Geräten – bitte beide aktualisieren.")
    } else if hello.code != code {
        ev(ServerEvent { peer: peer_s.clone(), kind: "abgewiesen".into() });
        deny("Falscher Code – bitte den Code von der Gegenstelle eingeben.")
    } else {
        Reply { ok: true, name: name.into(), error: String::new() }
    };
    let mut line = serde_json::to_vec(&reply)?;
    line.push(b'\n');
    s.write_all(&line).await?;
    if !reply.ok {
        return Ok(());
    }

    let secs = hello.secs.clamp(1.0, 60.0);
    match hello.kind.as_str() {
        "down" => {
            ev(ServerEvent { peer: peer_s, kind: "download".into() });
            let block = net::random_block();
            let end = Instant::now() + Duration::from_secs_f64(secs);
            while Instant::now() < end {
                if s.write_all(&block).await.is_err() {
                    break;
                }
            }
            let _ = s.shutdown().await;
        }
        "up" => {
            ev(ServerEvent { peer: peer_s, kind: "upload".into() });
            let (mut rd, mut wr) = s.into_split();
            let mut buf = vec![0u8; net::BUF];
            let mut total = 0u64;
            let mut tick = tokio::time::interval(Duration::from_millis(250));
            let end = tokio::time::Instant::now() + Duration::from_secs_f64(secs + 5.0);
            loop {
                tokio::select! {
                    r = rd.read(&mut buf) => match r {
                        Ok(0) | Err(_) => break,
                        Ok(n) => total += n as u64,
                    },
                    _ = tick.tick() => {
                        if wr.write_all(&total.to_be_bytes()).await.is_err() { break; }
                    }
                    _ = tokio::time::sleep_until(end) => break,
                }
            }
            let _ = wr.write_all(&total.to_be_bytes()).await;
        }
        _ => ev(ServerEvent { peer: peer_s, kind: "verbunden".into() }),
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct Found {
    pub name: String,
    pub addr: String,
}

/// Sucht Gegenstellen im lokalen Netz per Bonjour (wartet `wait`).
pub async fn discover(wait: Duration) -> Vec<Found> {
    tokio::task::spawn_blocking(move || {
        let mut out: Vec<Found> = Vec::new();
        let Ok(d) = mdns_sd::ServiceDaemon::new() else { return out };
        let Ok(rx) = d.browse(net::MDNS_TYPE) else { return out };
        let end = Instant::now() + wait;
        while let Some(left) = end.checked_duration_since(Instant::now()) {
            match rx.recv_timeout(left) {
                Ok(mdns_sd::ServiceEvent::ServiceResolved(info)) => {
                    let name = info.get_fullname().split('.').next().unwrap_or("").replace("\\032", " ");
                    for ip in info.get_addresses_v4() {
                        let addr = format!("{ip}:{}", info.get_port());
                        if !out.iter().any(|f| f.addr == addr) {
                            out.push(Found { name: name.clone(), addr });
                        }
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
        let _ = d.shutdown();
        out
    })
    .await
    .unwrap_or_default()
}
