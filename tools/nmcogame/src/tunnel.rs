//! The tunnel to the server's messenger service (B5 MS2, docs/messenger-design.md): one TCP
//! connection opened on the first call that needs it, frames `u32 LE length`, `u8 kind`, payload
//! (1 Call {seq, code, envelope}, 2 Reply {seq, ok, envelope}, 3 Event {target, code, envelope}).
//! A call waits for its Reply; a reader thread hands replies to their callers and posts events to
//! the window registered for their target. Safe code: posting is a function the `ffi` module
//! provides.
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    sync::{Mutex, OnceLock, PoisonError, mpsc},
    time::Duration,
};

/// Used when `nmcogame.ini` has no `address=` line.
pub const DEFAULT_ADDRESS: &str = "127.0.0.1:47611";
const CONNECT: Duration = Duration::from_secs(2);
const REPLY: Duration = Duration::from_secs(5);
/// The largest frame payload (the server's bound).
const MAX_FRAME: usize = 0x10_0000 + 64;
/// Event MsgConnectionClosed, posted to the game window when the tunnel drops.
const EVENT_CONNECTION_CLOSED: u32 = 0x1002;

/// Posts an event buffer to a window: (hWnd, message, event code, envelope) → posted.
pub type Post = fn(u32, u32, u32, Vec<u8>) -> bool;

#[derive(Default)]
struct State {
    stream: Option<TcpStream>,
    generation: u64,
    next_seq: u32,
    waiting: HashMap<u32, mpsc::Sender<(bool, Vec<u8>)>>,
    /// Callback target → (hWnd, message); 0 is the game window of RegisterCallback.
    targets: HashMap<u32, (u32, u32)>,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();
static POST: OnceLock<Post> = OnceLock::new();

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

pub fn set_post(post: Post) {
    let _ = POST.set(post);
}

/// A frame with its length prefix.
pub fn frame(kind: u8, a: u32, b: u32, envelope: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(envelope.len() + 9);
    payload.push(kind);
    payload.extend_from_slice(&a.to_le_bytes());
    if kind == 2 {
        payload.push(u8::from(b != 0));
    } else {
        payload.extend_from_slice(&b.to_le_bytes());
    }
    payload.extend_from_slice(envelope);
    let mut out = (payload.len() as u32).to_le_bytes().to_vec();
    out.extend_from_slice(&payload);
    out
}

/// A received frame: (kind, a, b, envelope); Reply's b is its ok byte.
pub fn parse(payload: &[u8]) -> Option<(u8, u32, u32, Vec<u8>)> {
    let (&kind, rest) = payload.split_first()?;
    let word = |at: usize| {
        rest.get(at..at + 4)
            .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
    };
    match kind {
        2 => Some((2, word(0)?, u32::from(*rest.get(4)?), rest.get(5..)?.to_vec())),
        3 => Some((3, word(0)?, word(4)?, rest.get(8..)?.to_vec())),
        _ => None,
    }
}

/// `address=host:port` from `nmcogame.ini` text (any section), else the default.
pub fn address(ini: Option<&str>) -> String {
    ini.and_then(|text| {
        text.lines().find_map(|line| {
            let (key, value) = line.split_once('=')?;
            (key.trim().eq_ignore_ascii_case("address") && !value.trim().is_empty())
                .then(|| value.trim().to_owned())
        })
    })
    .unwrap_or_else(|| DEFAULT_ADDRESS.to_owned())
}

fn connect() -> Option<TcpStream> {
    let ini = std::fs::read_to_string("nmcogame.ini").ok();
    let target: SocketAddr = address(ini.as_deref()).to_socket_addrs().ok()?.next()?;
    let stream = TcpStream::connect_timeout(&target, CONNECT).ok()?;
    let _ = stream.set_nodelay(true);
    Some(stream)
}

fn read_frame(stream: &mut TcpStream) -> Option<Vec<u8>> {
    let mut len = [0u8; 4];
    stream.read_exact(&mut len).ok()?;
    let len = u32::from_le_bytes(len) as usize;
    if len == 0 || len > MAX_FRAME {
        return None;
    }
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).ok()?;
    Some(payload)
}

fn reader(mut stream: TcpStream, generation: u64) {
    while let Some(payload) = read_frame(&mut stream) {
        match parse(&payload) {
            Some((2, seq, ok, envelope)) => {
                if let Some(waiter) = state().waiting.remove(&seq) {
                    let _ = waiter.send((ok != 0, envelope));
                }
            }
            Some((3, target, code, envelope)) => {
                let window = state().targets.get(&target).copied();
                if let (Some((hwnd, msg)), Some(post)) = (window, POST.get()) {
                    post(hwnd, msg, code, envelope);
                }
            }
            _ => break,
        }
    }
    let window = {
        let mut s = state();
        if s.generation != generation {
            return;
        }
        s.stream = None;
        s.waiting.clear();
        s.targets.get(&0).copied()
    };
    crate::log("tunnel closed");
    if let (Some((hwnd, msg)), Some(post)) = (window, POST.get()) {
        post(hwnd, msg, EVENT_CONNECTION_CLOSED, crate::event_reply(0));
    }
}

/// Sends a call through the tunnel and waits for its reply: None when the server cannot be
/// reached or does not answer in time.
pub fn call(code: u32, envelope: &[u8]) -> Option<(bool, Vec<u8>)> {
    let (tx, rx) = mpsc::channel();
    {
        let mut s = state();
        if s.stream.is_none() {
            let stream = connect()?;
            let read = stream.try_clone().ok()?;
            s.generation += 1;
            let generation = s.generation;
            std::thread::Builder::new()
                .name("nmcogame-tunnel".to_owned())
                .spawn(move || reader(read, generation))
                .ok()?;
            s.stream = Some(stream);
            crate::log("tunnel connected");
        }
        s.next_seq = s.next_seq.wrapping_add(1);
        let seq = s.next_seq;
        s.waiting.insert(seq, tx);
        let bytes = frame(1, seq, code, envelope);
        let written = s.stream.as_mut().map(|w| w.write_all(&bytes));
        if !matches!(written, Some(Ok(()))) {
            if let Some(stream) = s.stream.take() {
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
            s.waiting.remove(&seq);
            return None;
        }
    }
    rx.recv_timeout(REPLY).ok()
}

/// Keeps the window of a callback registration: 0x1002 (callArg 0, hWnd, message) is target 0;
/// 0xc001 / 0xb001 register a session or room key (the field before hWnd, else the callArg).
pub fn register(code: u32, fields: &[u32]) {
    let n = fields.len();
    if n < 3 {
        return;
    }
    let (hwnd, msg) = (fields[n - 2], fields[n - 1]);
    let key = match code {
        0x1002 => 0,
        _ => fields[n - 3],
    };
    state().targets.insert(key, (hwnd, msg));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_and_ini_address() {
        let call = frame(1, 7, 0x110f, &[9]);
        assert_eq!(call, [10, 0, 0, 0, 1, 7, 0, 0, 0, 0x0f, 0x11, 0, 0, 9]);
        let reply = frame(2, 7, 1, &[5, 6]);
        assert_eq!(parse(&reply[4..]), Some((2, 7, 1, vec![5, 6])));
        let event = frame(3, 0, 0x1001, &[1]);
        assert_eq!(parse(&event[4..]), Some((3, 0, 0x1001, vec![1])));
        assert_eq!(parse(&call[4..]), None);
        assert_eq!(address(None), DEFAULT_ADDRESS);
        assert_eq!(address(Some("[messenger]\r\naddress = 10.0.0.5:47611\r\n")), "10.0.0.5:47611");
    }

    #[test]
    fn registrations_map_targets() {
        register(0x1002, &[0, 0x1234, 0x8001]);
        register(0xc001, &[0, 77, 0x5678, 0x8001]);
        let s = state();
        assert_eq!(s.targets.get(&0), Some(&(0x1234, 0x8001)));
        assert_eq!(s.targets.get(&77), Some(&(0x5678, 0x8001)));
    }
}
