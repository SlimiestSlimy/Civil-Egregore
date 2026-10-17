//! A world shown: a ticker -- whatever ticks a world that is no
//! host: a test, an in-bin test --
//! answers a window's asks between its ticks when the environment
//! says so ([`RENDER_VARIABLE`]), over a local socket; and the window's end of
//! that, a host whose world is whatever ticker's comes
//! (`docs/server.md`, "A world shown").

use super::frame::{copy_superchunk, frame_of, hot_in, ViewportFrame, Water};
use super::terrain::{self, TerrainRequest};
use super::{wire, Host, HostRequest};
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
pub const RENDER_VARIABLE: &str = "CIVIL_EGREGORE_RENDER";
/// The port of this machine the window showing tickers' worlds listens on.
pub const SHOWN_WORLDS_PORT: u16 = 47_811;
/// How long a ticker waits for the window it started to listen.
const WINDOW_OPENS_WITHIN: Duration = Duration::from_secs(20);

/// Whether the environment asks for tickers' worlds to be shown.
pub fn worlds_shown_asked() -> bool {
    std::env::var(RENDER_VARIABLE).is_ok_and(|variable_value| !variable_value.is_empty() && variable_value != "0")
}

/// The ticker's end: the window its worlds are shown in, and what the window
/// last told it.
struct TickerEnd {
    /// The window.
    window: TcpStream,
    /// What the window asks, read on a thread of its own.
    request_receiver: Receiver<HostRequest>,
    /// Whether the window paused the ticker.
    ticking_paused: bool,
    /// Ticks a second the window holds it to: flat out, unless told.
    pace_held_to: Option<u32>,
    /// When the next tick is due, if paced.
    next_tick: Instant,
    /// Counts the worlds shown, and which one the last was: where it
    /// is in memory, and its seed.
    worlds: (u64, usize, u64),
    /// The superchunks of that world whose heights a frame carried.
    heights_sent: HashMap<SuperchunkIndex, Arc<Water>>,
    /// When the last frame was answered, and the tick then.
    last_frame: (Instant, u64),
}

/// The window this process's worlds are shown in: none if none was
/// asked for, or it could not be reached, or it has gone.
static TICKER_END: OnceLock<Mutex<Option<TickerEnd>>> = OnceLock::new();

/// The game's own program, to start as the window: this one, or the
/// one beside the folder a test's program is built in.
fn game_program() -> Option<std::path::PathBuf> {
    let running_program = std::env::current_exe().ok()?;
    let folder = running_program.parent()?;
    let program_name = format!("Civil_Egregore{}", std::env::consts::EXE_SUFFIX);
    [folder.join(&program_name), folder.parent()?.join(&program_name)].into_iter().find(|candidate_program| candidate_program.is_file())
}

/// The window listening, if there is one or one can be started.
fn connect_window() -> Option<TcpStream> {
    let connect_once = || TcpStream::connect((Ipv4Addr::LOCALHOST, SHOWN_WORLDS_PORT)).ok();
    if let Some(window) = connect_once() {
        return Some(window);
    }
    // None yet: the game's program is started as one, and shows no world of its own.
    let Some(game_program_path) = game_program() else {
        eprintln!("{RENDER_VARIABLE} is set, and the game's program is not built beside this one: no window to show the world in");
        return None;
    };
    if let Err(spawn_error) = Command::new(&game_program_path).args(["renderer", "show"]).env_remove(RENDER_VARIABLE).spawn() {
        eprintln!("{RENDER_VARIABLE} is set, and {} did not start: {spawn_error}", game_program_path.display());
        return None;
    }
    let wait_until = Instant::now() + WINDOW_OPENS_WITHIN;
    while Instant::now() < wait_until {
        if let Some(window) = connect_once() {
            return Some(window);
        }
        thread::sleep(Duration::from_millis(100));
    }
    eprintln!("{RENDER_VARIABLE} is set, and no window listened within {} seconds", WINDOW_OPENS_WITHIN.as_secs());
    None
}

impl TickerEnd {
    /// The ticker's end of a window reached, if one is.
    fn begin_showing() -> Option<Self> {
        let window = connect_window()?;
        _ = window.set_nodelay(true);
        let (request_sender, request_receiver) = channel();
        let mut window_reader = window.try_clone().ok()?;
        thread::Builder::new()
            .name("shown".to_string())
            .spawn(move || {
                while let Ok(words) = wire::receive_message(&mut window_reader) {
                    if wire::request_read(&words).is_some_and(|read_request| request_sender.send(read_request).is_err()) {
                        break;
                    }
                }
            })
            .ok()?;
        Some(Self { window, request_receiver, ticking_paused: false, pace_held_to: None, next_tick: Instant::now(), worlds: (0, 0, 0), heights_sent: HashMap::new(), last_frame: (Instant::now(), 0) })
    }

    /// What the window asked since the last tick, answered, and its
    /// pause and pace kept to: whether the window is still there.
    fn between_ticks(&mut self, world: &World) -> bool {
        let world_identity = (std::ptr::from_ref(world) as usize, world.world_info.seed);
        if (self.worlds.1, self.worlds.2) != world_identity {
            // Another world than the last shown: the window makes its picture anew.
            self.worlds = (self.worlds.0 + 1, world_identity.0, world_identity.1);
            self.heights_sent.clear();
            self.last_frame = (Instant::now(), world.entity_store.current_tick());
        }
        loop {
            // Paused, there is nothing to do until the window says more.
            let window_request = if self.ticking_paused { self.request_receiver.recv().map_err(|_| TryRecvError::Disconnected) } else { self.request_receiver.try_recv() };
            match window_request {
                Ok(HostRequest::AskFrame(frame_ask)) => {
                    let copying_began = Instant::now();
                    let hot = hot_in(world, frame_ask.viewport);
                    let cells = hot.iter().copied().skip(frame_ask.superchunks_skipped as usize).take(frame_ask.most_superchunks as usize).filter_map(|superchunk_cartesian| copy_superchunk(world, superchunk_cartesian, frame_ask, &mut self.heights_sent)).collect();
                    let (tick, copying_seconds, seconds_since_frame) = (world.entity_store.current_tick(), copying_began.elapsed().as_secs_f64(), self.last_frame.0.elapsed().as_secs_f64().max(f64::MIN_POSITIVE));
                    let frame_measured = ((tick - self.last_frame.1.min(tick)) as f64 / seconds_since_frame, copying_seconds, copying_seconds / seconds_since_frame);
                    self.last_frame = (Instant::now(), tick);
                    let answered_frame = frame_of(world, self.worlds.0, frame_ask, hot, cells, frame_measured, (None, None), false);
                    if wire::send_message(&mut self.window, &wire::frame_written(&answered_frame)).is_err() {
                        return false;
                    }
                }
                Ok(HostRequest::SetPaused(pause_asked)) => (self.ticking_paused, self.next_tick) = (pause_asked, Instant::now()),
                Ok(HostRequest::SetPace(pace_asked)) => (self.pace_held_to, self.next_tick) = (pace_asked, Instant::now()),
                Ok(_) => {}
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return false,
            }
        }
        if let Some(ticks_a_second) = self.pace_held_to {
            self.next_tick += Duration::from_secs_f64(1.0 / f64::from(ticks_a_second.max(1)));
            let time_now = Instant::now();
            match self.next_tick.checked_duration_since(time_now) {
                Some(wait_time) => thread::sleep(wait_time),
                None => self.next_tick = time_now,
            }
        }
        true
    }
}

/// Between two ticks of `world`, by whoever ticks it: if the
/// environment asks for tickers' worlds to be shown ([`RENDER_VARIABLE`]), what the
/// window asked is answered -- a frame of this world -- and its pause
/// and pace kept to; flat out unless it says otherwise. Nothing of
/// the world is changed by it, and nothing at all is done, but one
/// read of a flag, where no window was asked for.
#[inline]
pub(crate) fn between_ticks(world: &World) {
    static SHOWING_ASKED: OnceLock<bool> = OnceLock::new();
    if *SHOWING_ASKED.get_or_init(worlds_shown_asked) {
        serve_window(world);
    }
}

/// [`between_ticks`], a window having been asked for.
#[cold]
fn serve_window(world: &World) {
    let mut ticker_end_guard = TICKER_END.get_or_init(|| Mutex::new(TickerEnd::begin_showing())).lock().unwrap_or_else(|poisoned_lock| poisoned_lock.into_inner());
    if ticker_end_guard.as_mut().is_some_and(|ticker_end| !ticker_end.between_ticks(world)) {
        // The window closed: the ticker goes on unseen.
        *ticker_end_guard = None;
    }
}

/// What the window showing tickers' worlds was last told, for the
/// ticker that comes next: its pause, its pace, and the frame asked for and not
/// answered.
#[derive(Default)]
struct WindowTold {
    /// The ticker connected, if one is.
    ticker: Option<TcpStream>,
    /// Whether tickers are paused.
    ticking_paused: bool,
    /// The pace they are held to, if told one.
    pace_held_to: Option<Option<u32>>,
    /// The last frame asked for.
    last_frame_ask: Option<HostRequest>,
}

impl WindowTold {
    /// Sends `window_request` to the ticker connected, if one is: let go, if it
    /// has gone.
    fn send_to_ticker(&mut self, window_request: &HostRequest) {
        let (Some(ticker), Some(words)) = (&mut self.ticker, wire::request_written(window_request)) else {
            return;
        };
        if wire::send_message(ticker, &words).is_err() {
            self.ticker = None;
        }
    }
}

impl Host {
    /// A host whose world is whatever ticker's comes to be shown: it
    /// listens on [`SHOWN_WORLDS_PORT`], and each ticker that connects -- one at a
    /// time -- is asked what the window asks, its frames coming back
    /// as a host's do. It makes, opens and saves no world: those are
    /// the ticker's. `None` if the port is taken -- a window showing
    /// tickers' worlds is open already.
    pub fn of_shown_worlds() -> Option<(Self, Receiver<ViewportFrame>)> {
        let ticker_listener = TcpListener::bind((Ipv4Addr::LOCALHOST, SHOWN_WORLDS_PORT)).ok()?;
        let (request_sender, request_receiver) = channel::<HostRequest>();
        let (frame_sender, frame_receiver) = channel();
        let terrain = terrain::start_terrain_thread();
        let window_told: Arc<Mutex<WindowTold>> = Arc::default();
        let window_telling = Arc::clone(&window_told);
        // What the window asks goes to the ticker connected, and is kept for the next.
        thread::Builder::new().name("asking tickers".to_string()).spawn(move || {
            for window_request in request_receiver {
                let mut told_guard = window_telling.lock().unwrap_or_else(|poisoned_lock| poisoned_lock.into_inner());
                match &window_request {
                    HostRequest::SetPaused(pause_asked) => told_guard.ticking_paused = *pause_asked,
                    HostRequest::SetPace(pace_asked) => told_guard.pace_held_to = Some(*pace_asked),
                    HostRequest::AskFrame(_) => told_guard.last_frame_ask = Some(window_request.clone()),
                    _ => {}
                }
                told_guard.send_to_ticker(&window_request);
            }
        }).expect("a thread to ask tickers");
        let of_worlds = terrain.clone();
        thread::Builder::new().name("tickers".to_string()).spawn(move || tickers(&ticker_listener, &window_told, &frame_sender, &of_worlds)).expect("a thread for tickers");
        Some((Self { request_sender, terrain: terrain::TerrainAsker { request_sender: terrain } }, frame_receiver))
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

/// The next ticker to connect to `ticker_listener`: waited for as long as
/// it takes if none has been `shown_before` yet, else for
/// [`WINDOW_WAITS_FOR_THE_NEXT_TICKER`] and no longer.
fn next_ticker(ticker_listener: &TcpListener, shown_before: bool) -> Option<TcpStream> {
    if !shown_before {
        return ticker_listener.accept().ok().map(|(ticker, _)| ticker);
    }
    ticker_listener.set_nonblocking(true).ok()?;
    let wait_until = Instant::now() + WINDOW_WAITS_FOR_THE_NEXT_TICKER;
    let mut ticker = None;
    while ticker.is_none() && Instant::now() < wait_until {
        ticker = ticker_listener.accept().ok().map(|(ticker, _)| ticker);
        if ticker.is_none() {
            thread::sleep(Duration::from_millis(20));
        }
    }
    ticker_listener.set_nonblocking(false).ok()?;
    // A stream may come as its listener was: read as any other, waiting.
    ticker.filter(|ticker| ticker.set_nonblocking(false).is_ok())
}

/// Every ticker that connects to `ticker_listener`, one after another: told
/// what the window last said, its frames read and sent on to
/// `frame_sender` -- each world of each ticker a world of its own to the
/// window, the terrain's thread told of it first -- until the window
/// is gone, or the last ticker is and none comes after it
/// ([`no_ticker_is_left`]).
fn tickers(ticker_listener: &TcpListener, window_told: &Mutex<WindowTold>, frame_sender: &Sender<ViewportFrame>, terrain: &Sender<TerrainRequest>) {
    // Counts the worlds shown, over every ticker; and which one of the ticker connected the last was.
    let (mut worlds, mut shown_before) = (0, false);
    while let Some(ticker) = next_ticker(ticker_listener, shown_before) {
        shown_before = true;
        _ = ticker.set_nodelay(true);
        let Ok(mut ticker_reader) = ticker.try_clone() else {
            continue;
        };
        {
            let mut told_guard = window_told.lock().unwrap_or_else(|poisoned_lock| poisoned_lock.into_inner());
            told_guard.ticker = Some(ticker);
            let requests_again: Vec<HostRequest> = [told_guard.ticking_paused.then_some(HostRequest::SetPaused(true)), told_guard.pace_held_to.map(HostRequest::SetPace), told_guard.last_frame_ask.clone()].into_iter().flatten().collect();
            requests_again.iter().for_each(|request_again| told_guard.send_to_ticker(request_again));
        }
        let (mut waters, mut last_world_number) = (HashMap::new(), None);
        while let Ok(words) = wire::receive_message(&mut ticker_reader) {
            let Some(mut read_frame) = wire::frame_read(&words, &mut waters) else {
                break;
            };
            if last_world_number != Some(read_frame.world) {
                (worlds, last_world_number) = (worlds + 1, Some(read_frame.world));
                _ = terrain.send(TerrainRequest::World(worlds, read_frame.seed, Box::new(read_frame.world_generation)));
            }
            read_frame.world = worlds;
            if frame_sender.send(read_frame).is_err() {
                return;
            }
        }
        window_told.lock().unwrap_or_else(|poisoned_at_end| poisoned_at_end.into_inner()).ticker = None;
    }
    NO_TICKER_LEFT.store(true, Ordering::Relaxed);
}
