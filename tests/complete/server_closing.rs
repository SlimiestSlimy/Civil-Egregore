//! A server told to close if no clients, as a window starts it: closed
//! once none has been connected for its wait, and not before.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long a server told to close if no clients waits for one: the
/// server's `CLIENT_WAIT`, as its flag's help says it.
const CLIENT_WAIT: Duration = Duration::from_secs(30);

/// Started to close if no clients and never connected to, the server
/// says where it listens, waits its while, and closes well -- no
/// sooner. The one port: fails if another server is running.
#[test]
#[ignore]
fn a_server_told_to_close_if_no_clients_closes_once_none_came() {
    let mut server = Command::new(env!("CARGO_BIN_EXE_Civil_Egregore")).arg("--close_if_no_clients").stdin(Stdio::null()).stdout(Stdio::piped()).spawn().expect("the server started");
    let said = BufReader::new(server.stdout.take().expect("its stdout")).lines();
    assert!(said.map_while(Result::ok).any(|line| line.starts_with("listening on ")), "where it listens, said");
    let listening_since = Instant::now();
    let exit_status = server.wait().expect("the server ended");
    let waited = listening_since.elapsed();
    assert!(exit_status.success() && waited >= CLIENT_WAIT - Duration::from_secs(1) && waited < CLIENT_WAIT + Duration::from_secs(10), "closed after {waited:?}: {exit_status}");
}
