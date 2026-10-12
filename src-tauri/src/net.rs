//! Sockets fest an die gewählte Schnittstelle binden, plus das kleine Protokoll zur Gegenstelle.
//!
//! Protokoll (TCP, Port 5299): Der Client schickt eine JSON-Zeile
//!   {"v":1,"code":"4711","kind":"hello"|"down"|"up","secs":10.5}
//! und bekommt eine JSON-Zeile zurück: {"ok":true,"name":"Praxis-PC"} bzw. {"ok":false,"error":"…"}.
//!   down: Gegenstelle sendet Zufallsdaten bis secs abgelaufen sind, dann schließt sie.
//!   up:   Client sendet; die Gegenstelle meldet alle 250 ms die bisher empfangenen Bytes als u64 (big endian)
//!         zurück - so zählt beim Upload, was wirklich angekommen ist, nicht was im Sendepuffer liegt.
//! UDP (selber Port): "NSPT" + Code (4 Byte) + Folgenummer u32 + Zeitstempel - wird unverändert zurückgeschickt.

use serde::{Deserialize, Serialize};
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpSocket, TcpStream, UdpSocket};

pub const PORT: u16 = 5299;
pub const VERSION: u32 = 1;
pub const MDNS_TYPE: &str = "_netzspeed._tcp.local.";
pub const UDP_MAGIC: &[u8; 4] = b"NSPT";
pub const BUF: usize = 256 * 1024;
pub const SOCK_BUF: usize = 4 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
pub struct Hello {
    pub v: u32,
    pub code: String,
    pub kind: String,
    #[serde(default)]
    pub secs: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Reply {
    pub ok: bool,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub error: String,
}

/// Wohin die Sockets gebunden werden: Quell-IP der Schnittstelle und (macOS/Windows) ihr Index.
#[derive(Debug, Clone, Copy, Default)]
pub struct Bind {
    pub ip: Option<Ipv4Addr>,
    pub index: Option<u32>,
}

#[cfg(unix)]
type Raw<'a> = std::os::fd::BorrowedFd<'a>;
#[cfg(windows)]
type Raw<'a> = std::os::windows::io::BorrowedSocket<'a>;

/// Bindet den Socket an die Schnittstelle, unabhängig von der Routing-Tabelle.
/// Nur an die IP zu binden reicht nicht immer, wenn Kabel und WLAN gleichzeitig aktiv sind.
fn bind_device(sock: Raw<'_>, bind: &Bind) -> io::Result<()> {
    let Some(index) = bind.index else { return Ok(()) };
    #[cfg(target_os = "macos")]
    {
        socket2::SockRef::from(&sock).bind_device_by_index_v4(std::num::NonZeroU32::new(index))?;
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawSocket;
        use windows::Win32::Networking::WinSock::{IP_UNICAST_IF, IPPROTO_IP, SOCKET, setsockopt};
        // IP_UNICAST_IF erwartet den Index in Netzwerk-Byte-Reihenfolge
        let v = index.to_be().to_ne_bytes();
        let rc = unsafe { setsockopt(SOCKET(sock.as_raw_socket() as usize), IPPROTO_IP.0, IP_UNICAST_IF, Some(&v)) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    let _ = (sock, index);
    Ok(())
}

#[cfg(unix)]
fn raw<T: std::os::fd::AsFd>(s: &T) -> Raw<'_> { s.as_fd() }
#[cfg(windows)]
fn raw<T: std::os::windows::io::AsSocket>(s: &T) -> Raw<'_> { s.as_socket() }

pub async fn tcp_connect(addr: SocketAddr, bind: &Bind) -> io::Result<TcpStream> {
    let sock = TcpSocket::new_v4()?;
    let _ = sock.set_send_buffer_size(SOCK_BUF as u32);
    let _ = sock.set_recv_buffer_size(SOCK_BUF as u32);
    bind_device(raw(&sock), bind)?;
    if let Some(ip) = bind.ip {
        sock.bind(SocketAddr::from((ip, 0)))?;
    }
    let s = tokio::time::timeout(Duration::from_secs(5), sock.connect(addr))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, format!("Gegenstelle {addr} antwortet nicht")))??;
    s.set_nodelay(true)?;
    Ok(s)
}

pub async fn udp_bound(bind: &Bind) -> io::Result<UdpSocket> {
    let s = socket2::Socket::new(socket2::Domain::IPV4, socket2::Type::DGRAM, Some(socket2::Protocol::UDP))?;
    bind_device(raw(&s), bind)?;
    s.bind(&SocketAddr::from((bind.ip.unwrap_or(Ipv4Addr::UNSPECIFIED), 0)).into())?;
    s.set_nonblocking(true)?;
    UdpSocket::from_std(s.into())
}

/// Handshake auf einer frischen Verbindung: Zeile senden, Antwortzeile lesen.
/// Gibt den Stream ohne Lesepuffer-Reste zurück (die Gegenstelle sendet erst nach der Antwort Nutzdaten,
/// und beim Download liest der Client die Antwort Byte für Byte bis zum Zeilenende).
pub async fn handshake(mut s: TcpStream, hello: &Hello) -> io::Result<(TcpStream, Reply)> {
    let mut line = serde_json::to_vec(hello)?;
    line.push(b'\n');
    s.write_all(&line).await?;
    let mut reply = Vec::with_capacity(128);
    let mut b = [0u8; 1];
    loop {
        let n = tokio::time::timeout(Duration::from_secs(5), tokio::io::AsyncReadExt::read(&mut s, &mut b))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "keine Antwort der Gegenstelle"))??;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Gegenstelle hat die Verbindung beendet"));
        }
        if b[0] == b'\n' {
            break;
        }
        if reply.len() > 4096 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "ungültige Antwort"));
        }
        reply.push(b[0]);
    }
    let r: Reply = serde_json::from_slice(&reply).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    if !r.ok {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, r.error.clone()));
    }
    Ok((s, r))
}

/// Liest die Hello-Zeile auf der Server-Seite (max. 4 KB, 5 s).
pub async fn read_hello(s: &mut TcpStream) -> io::Result<Hello> {
    let mut r = BufReader::new(s);
    let mut line = String::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut take = (&mut r).take(4096);
        take.read_line(&mut line).await
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "kein Hello"))??;
    // Client sendet nichts vor unserer Antwort -> der BufReader hat nichts Weiteres gepuffert.
    serde_json::from_str(line.trim()).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

use tokio::io::AsyncReadExt;

/// Nicht komprimierbare Nutzdaten (einmal erzeugt, dann immer wieder gesendet).
pub fn random_block() -> bytes::Bytes {
    use rand::RngCore;
    let mut v = vec![0u8; BUF];
    rand::rng().fill_bytes(&mut v);
    bytes::Bytes::from(v)
}

pub fn parse_server(s: &str) -> io::Result<SocketAddr> {
    let s = s.trim();
    if let Ok(a) = s.parse::<SocketAddr>() {
        return Ok(a);
    }
    if let Ok(ip) = s.parse::<Ipv4Addr>() {
        return Ok(SocketAddr::from((ip, PORT)));
    }
    Err(io::Error::new(io::ErrorKind::InvalidInput, format!("„{s}“ ist keine gültige IP-Adresse")))
}
