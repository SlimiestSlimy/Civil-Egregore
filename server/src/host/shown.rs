//! A world shown: a ticker -- whatever ticks a world that is no
//! host: a test, an in-bin test --
//! answers a window's asks between its ticks when the environment
//! says so ([`RENDER`]), over a local socket; and the window's end of
//! that, a host whose world is whatever ticker's comes
//! (`docs/server.md`, "A world shown").

use super::frame::{copy, frame_of, hot_in, Frame, Water};
use super::terrain::{self, TerrainRequest};
use super::{wire, Host, Request};
use crate::World;
use coordinates::SuperchunkIndex;
use std::collections::HashMap;
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

/// The environment variable that has every world a ticker ticks
/// shown in a window: set to anything but `0` or nothing.
pub const RENDER: &str = "CIVIL_EGREGORE_RENDER";
/// The port of this machine the window showing tickers' worlds listens on.
pub const PORT: u16 = 47_811;
/// How long a ticker waits for the window it started to listen.
const WINDOW_OPENS_WITHIN: Duration = Duration::from_secs(20);

/// Whether the environment asks for tickers' worlds to be shown.
pub fn asked() -> bool {
    std::env::var(RENDER).is_ok_and(|said| !said.is_empty() && said != "0")
}

/// The ticker's end: the window its worlds are shown in, and what the window
/// last told it.
struct Shown {
    /// The window.
    window: TcpStream,
    /// What the window asks, read on a thread of its own.
    requests: Receiver<Request>,
    /// Whether the window paused the ticker.
    paused: bool,
    /// Ticks a second the window holds it to: flat out, unless told.
    pace: Option<u32>,
    /// When the next tick is due, if paced.
    next_tick: Instant,
    /// Counts the worlds shown, and which one the last was: where it
    /// is in memory, and its seed.
    worlds: (u64, usize, u64),
    /// The superchunks of that world whose heights a frame carried.
    sent: HashMap<SuperchunkIndex, Arc<Water>>,
    /// When the last frame was answered, and the tick then.
    last_frame: (Instant, u64),
}

/// The window this process's worlds are shown in: none if none was
/// asked for, or it could not be reached, or it has gone.
static SHOWN: OnceLock<Mutex<Option<Shown>>> = OnceLock::new();

/// The game's own program, to start as the window: this one, or the
/// one beside the folder a test's program is built in.
fn program() -> Option<std::path::PathBuf> {
    let running = std::env::current_exe().ok()?;
    let folder = running.parent()?;
    let named = format!("Civil_Egregore{}", std::env::consts::EXE_SUFFIX);
    [folder.join(&named), folder.parent()?.join(&named)].into_iter().find(|program| program.is_file())
}

/// The window listening, if there is one or one can be started.
fn connect_window() -> Option<TcpStream> {
    let connect = || TcpStream::connect((Ipv4Addr::LOCALHOST, PORT)).ok();
    if let Some(window) = connect() {
        return Some(window);
    }
    // None yet: the game's program is started as one, and shows no world of its own.
    let Some(program) = program() else {
        eprintln!("{RENDER} is set, and the game's program is not built beside this one: no window to show the world in");
        return None;
    };
    if let Err(why) = Command::new(&program).args(["renderer", "show"]).env_remove(RENDER).spawn() {
        eprintln!("{RENDER} is set, and {} did not start: {why}", program.display());
        return None;
    }
    let until = Instant::now() + WINDOW_OPENS_WITHIN;
    while Instant::now() < until {
        if let Some(window) = connect() {
            return Some(window);
        }
        thread::sleep(Duration::from_millis(100));
    }
    eprintln!("{RENDER} is set, and no window listened within {} seconds", WINDOW_OPENS_WITHIN.as_secs());
    None
}

impl Shown {
    /// The ticker's end of a window reached, if one is.
    fn begin() -> Option<Self> {
        let window = connect_window()?;
        _ = window.set_nodelay(true);
        let (told, requests) = channel();
        let mut from = window.try_clone().ok()?;
        thread::Builder::new()
            .name("shown".to_string())
            .spawn(move || {
                while let Ok(words) = wire::receive(&mut from) {
                    if wire::request_read(&words).is_some_and(|request| told.send(request).is_err()) {
                        break;
                    }
                }
            })
            .ok()?;
        Some(Self { window, requests, paused: false, pace: None, next_tick: Instant::now(), worlds: (0, 0, 0), sent: HashMap::new(), last_frame: (Instant::now(), 0) })
    }

    /// What the window asked since the last tick, answered, and its
    /// pause and pace kept to: whether the window is still there.
    fn between_ticks(&mut self, world: &World) -> bool {
        let which = (std::ptr::from_ref(world) as usize, world.info.seed);
        if (self.worlds.1, self.worlds.2) != which {
            // Another world than the last shown: the window makes its picture anew.
            self.worlds = (self.worlds.0 + 1, which.0, which.1);
            self.sent.clear();
            self.last_frame = (Instant::now(), world.entities.now());
        }
        loop {
            // Paused, there is nothing to do until the window says more.
            let request = if self.paused { self.requests.recv().map_err(|_| TryRecvError::Disconnected) } else { self.requests.try_recv() };
            match request {
                Ok(Request::Sync(ask)) => {
                    let began = Instant::now();
                    let hot = hot_in(world, ask.viewport);
                    let cells = hot.iter().copied().skip(ask.skip as usize).take(ask.most as usize).filter_map(|at| copy(world, at, ask, &mut self.sent)).collect();
                    let (tick, spent, elapsed) = (world.entities.now(), began.elapsed().as_secs_f64(), self.last_frame.0.elapsed().as_secs_f64().max(f64::MIN_POSITIVE));
                    let measured = ((tick - self.last_frame.1.min(tick)) as f64 / elapsed, spent, spent / elapsed);
                    self.last_frame = (Instant::now(), tick);
                    let frame = frame_of(world, self.worlds.0, ask, hot, cells, measured, (None, None), false);
                    if wire::send(&mut self.window, &wire::frame_written(&frame)).is_err() {
                        return false;
                    }
                }
                Ok(Request::Pause(paused)) => (self.paused, self.next_tick) = (paused, Instant::now()),
                Ok(Request::Pace(pace)) => (self.pace, self.next_tick) = (pace, Instant::now()),
                Ok(_) => {}
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return false,
            }
        }
        if let Some(pace) = self.pace {
            self.next_tick += Duration::from_secs_f64(1.0 / f64::from(pace.max(1)));
            let now = Instant::now();
            match self.next_tick.checked_duration_since(now) {
                Some(wait) => thread::sleep(wait),
                None => self.next_tick = now,
            }
        }
        true
    }
}

/// Between two ticks of `world`, by whoever ticks it: if the
/// environment asks for tickers' worlds to be shown ([`RENDER`]), what the
/// window asked is answered -- a frame of this world -- and its pause
/// and pace kept to; flat out unless it says otherwise. Nothing of
/// the world is changed by it, and nothing at all is done, but one
/// read of a flag, where no window was asked for.
#[inline]
pub(crate) fn between_ticks(world: &World) {
    static ASKED: OnceLock<bool> = OnceLock::new();
    if *ASKED.get_or_init(asked) {
        serve_window(world);
    }
}

/// [`between_ticks`], a window having been asked for.
#[cold]
fn serve_window(world: &World) {
    let mut shown = SHOWN.get_or_init(|| Mutex::new(Shown::begin())).lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if shown.as_mut().is_some_and(|shown| !shown.between_ticks(world)) {
        // The window closed: the ticker goes on unseen.
        *shown = None;
    }
}

/// What the window showing tickers' worlds was last told, for the
/// ticker that comes next: its pause, its pace, and the frame asked for and not
/// answered.
#[derive(Default)]
struct Told {
    /// The ticker connected, if one is.
    ticker: Option<TcpStream>,
    /// Whether tickers are paused.
    paused: bool,
    /// The pace they are held to, if told one.
    pace: Option<Option<u32>>,
    /// The last frame asked for.
    ask: Option<Request>,
}

impl Told {
    /// Sends `request` to the ticker connected, if one is: let go, if it
    /// has gone.
    fn send(&mut self, request: &Request) {
        let (Some(ticker), Some(words)) = (&mut self.ticker, wire::request_written(request)) else {
            return;
        };
        if wire::send(ticker, &words).is_err() {
            self.ticker = None;
        }
    }
}

impl Host {
    /// A host whose world is whatever ticker's comes to be shown: it
    /// listens on [`PORT`], and each ticker that connects -- one at a
    /// time -- is asked what the window asks, its frames coming back
    /// as a host's do. It makes, opens and saves no world: those are
    /// the ticker's. `None` if the port is taken -- a window showing
    /// tickers' worlds is open already.
    pub fn of_shown_worlds() -> Option<(Self, Receiver<Frame>)> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, PORT)).ok()?;
        let (requests, asked) = channel::<Request>();
        let (answers, frames) = channel();
        let terrain = terrain::start();
        let told: Arc<Mutex<Told>> = Arc::default();
        let telling = Arc::clone(&told);
        // What the window asks goes to the ticker connected, and is kept for the next.
        thread::Builder::new().name("asking tickers".to_string()).spawn(move || {
            for request in asked {
                let mut told = telling.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                match &request {
                    Request::Pause(paused) => told.paused = *paused,
                    Request::Pace(pace) => told.pace = Some(*pace),
                    Request::Sync(_) => told.ask = Some(request.clone()),
                    _ => {}
                }
                told.send(&request);
            }
        }).expect("a thread to ask tickers");
        let of_worlds = terrain.clone();
        thread::Builder::new().name("tickers".to_string()).spawn(move || tickers(&listener, &told, &answers, &of_worlds)).expect("a thread for tickers");
        Some((Self { requests, terrain: terrain::TerrainAsker { requests: terrain } }, frames))
    }
}

/// How long a window that has shown a ticker's world waits for the
/// next ticker once none is connected, before it says none is left
/// ([`no_ticker_is_left`]): the time one test program takes to end
/// and the next to start.
pub const WINDOW_WAITS_FOR_THE_NEXT_TICKER: Duration = Duration::from_secs(2);

/// Whether this process's window has shown a ticker and none has
/// come since: set by [`tickers`], asked by the window.
static NO_TICKER_LEFT: AtomicBool = AtomicBool::new(false);

/// Whether the tickers this process's window showed are all gone, and
/// none came in [`WINDOW_WAITS_FOR_THE_NEXT_TICKER`]: the window
/// closes then, as whatever ticked has stopped. False before the
/// first ticker: a window opened by hand waits for one.
pub fn no_ticker_is_left() -> bool {
    NO_TICKER_LEFT.load(Ordering::Relaxed)
}

/// The next ticker to connect to `listener`: waited for as long as
/// it takes if none has been `shown` yet, else for
/// [`WINDOW_WAITS_FOR_THE_NEXT_TICKER`] and no longer.
fn next_ticker(listener: &TcpListener, shown: bool) -> Option<TcpStream> {
    if !shown {
        return listener.accept().ok().map(|(ticker, _)| ticker);
    }
    listener.set_nonblocking(true).ok()?;
    let until = Instant::now() + WINDOW_WAITS_FOR_THE_NEXT_TICKER;
    let mut ticker = None;
    while ticker.is_none() && Instant::now() < until {
        ticker = listener.accept().ok().map(|(ticker, _)| ticker);
        if ticker.is_none() {
            thread::sleep(Duration::from_millis(20));
        }
    }
    listener.set_nonblocking(false).ok()?;
    // A stream may come as its listener was: read as any other, waiting.
    ticker.filter(|ticker| ticker.set_nonblocking(false).is_ok())
}

/// Every ticker that connects to `listener`, one after another: told
/// what the window last said, its frames read and sent on to
/// `answers` -- each world of each ticker a world of its own to the
/// window, the terrain's thread told of it first -- until the window
/// is gone, or the last ticker is and none comes after it
/// ([`no_ticker_is_left`]).
fn tickers(listener: &TcpListener, told: &Mutex<Told>, answers: &Sender<Frame>, terrain: &Sender<TerrainRequest>) {
    // Counts the worlds shown, over every ticker; and which one of the ticker connected the last was.
    let (mut worlds, mut shown) = (0, false);
    while let Some(ticker) = next_ticker(listener, shown) {
        shown = true;
        _ = ticker.set_nodelay(true);
        let Ok(mut from) = ticker.try_clone() else {
            continue;
        };
        {
            let mut told = told.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            told.ticker = Some(ticker);
            let again: Vec<Request> = [told.paused.then_some(Request::Pause(true)), told.pace.map(Request::Pace), told.ask.clone()].into_iter().flatten().collect();
            again.iter().for_each(|request| told.send(request));
        }
        let (mut waters, mut last) = (HashMap::new(), None);
        while let Ok(words) = wire::receive(&mut from) {
            let Some(mut frame) = wire::frame_read(&words, &mut waters) else {
                break;
            };
            if last != Some(frame.world) {
                (worlds, last) = (worlds + 1, Some(frame.world));
                _ = terrain.send(TerrainRequest::World(worlds, frame.seed, Box::new(frame.generation)));
            }
            frame.world = worlds;
            if answers.send(frame).is_err() {
                return;
            }
        }
        told.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).ticker = None;
    }
    NO_TICKER_LEFT.store(true, Ordering::Relaxed);
}
