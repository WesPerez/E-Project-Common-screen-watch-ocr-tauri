use std::{
    env,
    io::{self, Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub const INSTANCE_HOST: &str = "127.0.0.1";
pub const INSTANCE_PORT: u16 = 47628;
pub const INSTANCE_PORT_ENV: &str = "SCREENWATCH_TAURI_SINGLE_INSTANCE_PORT";
pub const INSTANCE_COMMAND: &[u8] = b"ScreenWatchOCRTauri:show\n";
pub const INSTANCE_ACK: &[u8] = b"ok\n";

const LISTENER_IDLE_SLEEP: Duration = Duration::from_millis(20);
const CONNECTION_TIMEOUT: Duration = Duration::from_millis(200);

#[derive(Debug)]
pub enum ClaimResult {
    NotifiedExisting,
    Listening(SingleInstanceGuard),
    Unavailable(io::Error),
}

#[derive(Debug)]
pub struct SingleInstanceGuard {
    #[cfg(test)]
    local_addr: SocketAddr,
    stop: Arc<AtomicBool>,
    join: Mutex<Option<JoinHandle<()>>>,
}

impl SingleInstanceGuard {
    pub fn listen<F>(listener: TcpListener, on_wake: F) -> io::Result<Self>
    where
        F: Fn() + Send + Sync + 'static,
    {
        listener.set_nonblocking(true)?;
        #[cfg(test)]
        let local_addr = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = Arc::clone(&stop);
        let on_wake: Arc<dyn Fn() + Send + Sync> = Arc::new(on_wake);
        let join = thread::Builder::new()
            .name("screen-watch-tauri-single-instance".to_string())
            .spawn(move || run_listener(listener, stop_for_thread, on_wake))
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))?;

        Ok(Self {
            #[cfg(test)]
            local_addr,
            stop,
            join: Mutex::new(Some(join)),
        })
    }

    #[cfg(test)]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Ok(mut join) = self.join.lock() {
            if let Some(handle) = join.take() {
                let _ = handle.join();
            }
        }
    }
}

pub fn default_instance_addr() -> SocketAddr {
    instance_addr_from_port_env(env::var(INSTANCE_PORT_ENV).ok().as_deref())
}

pub fn instance_addr_from_port_env(port_env: Option<&str>) -> SocketAddr {
    SocketAddr::new(
        INSTANCE_HOST
            .parse::<IpAddr>()
            .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        configured_instance_port(port_env),
    )
}

fn configured_instance_port(port_env: Option<&str>) -> u16 {
    port_env
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|port| *port != 0)
        .unwrap_or(INSTANCE_PORT)
}

pub fn notify_existing_instance_at(addr: SocketAddr, timeout: Duration) -> bool {
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, timeout) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    if stream.write_all(INSTANCE_COMMAND).is_err() {
        return false;
    }
    let mut ack = vec![0; INSTANCE_ACK.len()];
    stream.read_exact(&mut ack).is_ok() && ack == INSTANCE_ACK
}

pub fn claim_single_instance<F>(timeout: Duration, on_wake: F) -> ClaimResult
where
    F: Fn() + Send + Sync + 'static,
{
    claim_single_instance_at(default_instance_addr(), timeout, on_wake)
}

pub fn claim_single_instance_at<F>(addr: SocketAddr, timeout: Duration, on_wake: F) -> ClaimResult
where
    F: Fn() + Send + Sync + 'static,
{
    if notify_existing_instance_at(addr, timeout) {
        return ClaimResult::NotifiedExisting;
    }

    match TcpListener::bind(addr) {
        Ok(listener) => match SingleInstanceGuard::listen(listener, on_wake) {
            Ok(guard) => ClaimResult::Listening(guard),
            Err(err) => ClaimResult::Unavailable(err),
        },
        Err(err) if err.kind() == io::ErrorKind::AddrInUse => {
            if notify_existing_instance_at(addr, timeout) {
                ClaimResult::NotifiedExisting
            } else {
                ClaimResult::Unavailable(err)
            }
        }
        Err(err) => ClaimResult::Unavailable(err),
    }
}

fn run_listener(
    listener: TcpListener,
    stop: Arc<AtomicBool>,
    on_wake: Arc<dyn Fn() + Send + Sync>,
) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => handle_connection(stream, on_wake.as_ref()),
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(LISTENER_IDLE_SLEEP);
            }
            Err(_) => break,
        }
    }
}

fn handle_connection(mut stream: TcpStream, on_wake: &(dyn Fn() + Send + Sync)) {
    let _ = stream.set_read_timeout(Some(CONNECTION_TIMEOUT));
    let _ = stream.set_write_timeout(Some(CONNECTION_TIMEOUT));

    let mut message = Vec::with_capacity(INSTANCE_COMMAND.len());
    let mut byte = [0_u8; 1];
    while message.len() < 128 {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                message.push(byte[0]);
                if byte[0] == b'\n' {
                    break;
                }
            }
            Err(err)
                if err.kind() == io::ErrorKind::WouldBlock
                    || err.kind() == io::ErrorKind::TimedOut =>
            {
                break;
            }
            Err(_) => return,
        }
    }

    if message == INSTANCE_COMMAND {
        let _ = stream.write_all(INSTANCE_ACK);
        let _ = stream.flush();
        on_wake();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        claim_single_instance_at, instance_addr_from_port_env, notify_existing_instance_at,
        ClaimResult, SingleInstanceGuard, INSTANCE_ACK, INSTANCE_COMMAND, INSTANCE_PORT,
    };
    use std::{
        io::{Read, Write},
        net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        thread,
        time::{Duration, Instant},
    };

    const TEST_TIMEOUT: Duration = Duration::from_millis(500);

    #[test]
    fn instance_port_env_allows_isolated_packaged_smoke_runs() {
        assert_eq!(instance_addr_from_port_env(None).port(), INSTANCE_PORT);
        assert_eq!(instance_addr_from_port_env(Some("49152")).port(), 49152);
        assert_eq!(instance_addr_from_port_env(Some("0")).port(), INSTANCE_PORT);
        assert_eq!(
            instance_addr_from_port_env(Some("not-a-port")).port(),
            INSTANCE_PORT
        );
    }

    #[test]
    fn notify_existing_instance_sends_tauri_protocol_and_accepts_ack() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut command = vec![0; INSTANCE_COMMAND.len()];
            stream.read_exact(&mut command).unwrap();
            assert_eq!(command, INSTANCE_COMMAND);
            stream.write_all(INSTANCE_ACK).unwrap();
        });

        assert!(notify_existing_instance_at(addr, TEST_TIMEOUT));
        handle.join().unwrap();
    }

    #[test]
    fn claim_notifies_existing_listener_instead_of_binding_again() {
        let wake_count = Arc::new(AtomicUsize::new(0));
        let existing = listening_guard(Arc::clone(&wake_count));

        let result = claim_single_instance_at(existing.local_addr(), TEST_TIMEOUT, || {});

        assert!(matches!(result, ClaimResult::NotifiedExisting));
        wait_for_count(&wake_count, 1);
        drop(existing);
    }

    #[test]
    fn claim_listens_when_no_existing_instance_replies() {
        let wake_count = Arc::new(AtomicUsize::new(0));
        let result =
            claim_single_instance_at(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)), TEST_TIMEOUT, {
                let wake_count = Arc::clone(&wake_count);
                move || {
                    wake_count.fetch_add(1, Ordering::SeqCst);
                }
            });
        let ClaimResult::Listening(guard) = result else {
            panic!("expected listener claim");
        };

        assert_notified_with_retry(guard.local_addr());
        wait_for_count(&wake_count, 1);
        drop(guard);
    }

    #[test]
    fn claim_retries_notification_when_existing_port_is_busy_but_first_ack_is_missing() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            for attempt in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut command = vec![0; INSTANCE_COMMAND.len()];
                stream.read_exact(&mut command).unwrap();
                assert_eq!(command, INSTANCE_COMMAND);
                if attempt == 1 {
                    stream.write_all(INSTANCE_ACK).unwrap();
                }
            }
        });

        let result = claim_single_instance_at(addr, TEST_TIMEOUT, || {});

        assert!(matches!(result, ClaimResult::NotifiedExisting));
        handle.join().unwrap();
    }

    #[test]
    fn malformed_command_does_not_ack_or_wake() {
        let wake_count = Arc::new(AtomicUsize::new(0));
        let guard = listening_guard(Arc::clone(&wake_count));
        let mut stream = TcpStream::connect_timeout(&guard.local_addr(), TEST_TIMEOUT).unwrap();
        stream.set_read_timeout(Some(TEST_TIMEOUT)).unwrap();

        stream.write_all(b"not-this-app\n").unwrap();
        let mut ack = [0_u8; 3];
        let read_result = stream.read(&mut ack);

        assert!(read_result.is_err() || read_result.unwrap() == 0);
        assert_eq!(wake_count.load(Ordering::SeqCst), 0);
        drop(guard);
    }

    fn listening_guard(wake_count: Arc<AtomicUsize>) -> SingleInstanceGuard {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        SingleInstanceGuard::listen(listener, move || {
            wake_count.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap()
    }

    fn wait_for_count(counter: &AtomicUsize, expected: usize) {
        let deadline = Instant::now() + TEST_TIMEOUT;
        while Instant::now() < deadline {
            if counter.load(Ordering::SeqCst) == expected {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(counter.load(Ordering::SeqCst), expected);
    }

    fn assert_notified_with_retry(addr: SocketAddr) {
        let deadline = Instant::now() + TEST_TIMEOUT;
        while Instant::now() < deadline {
            if notify_existing_instance_at(addr, TEST_TIMEOUT) {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(notify_existing_instance_at(addr, TEST_TIMEOUT));
    }
}
