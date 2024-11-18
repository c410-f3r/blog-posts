mod ws;

use std::{collections::HashMap, sync::LazyLock, task::Waker};
use tokio::{io::WriteHalf, net::TcpStream, sync::Mutex};
use tokio_rustls::server::TlsStream;
use wtx::{
  http::{
    server_framework::{web_socket, Router, ServerFrameworkBuilder},
    Headers, ManualStream,
  },
  http2::{Http2Buffer, Http2DataTokio, WebSocketOverStream},
  misc::{simple_seed, Deque, Xorshift64},
};

static CERT: &[u8] = include_bytes!("../../../.certs/cert.pem");
static KEY: &[u8] = include_bytes!("../../../.certs/key.pem");
static USER_POOL: LazyLock<Mutex<UserPool>> =
  LazyLock::new(|| Mutex::const_new(UserPool { matching: Deque::new(), messages: HashMap::new() }));

type ServerStream = wtx::http2::ServerStream<Http2DataTokio<Http2Buffer, StreamWriter, false>>;
type StreamWriter = WriteHalf<TlsStream<TcpStream>>;

#[tokio::main]
async fn main() -> wtx::Result<()> {
  let router = Router::paths(wtx::paths!(("/chat", web_socket(chat)),))?;
  ServerFrameworkBuilder::new(router)
    .enable_connect_protocol(true)
    .max_hpack_len((128 * 1024, 128 * 1024))
    .without_aux()
    .tokio_rustls(
      (CERT, KEY),
      "0.0.0.0:9000",
      Xorshift64::from(simple_seed()),
      |error| eprintln!("{error:?}"),
      |_| Ok(()),
    )
    .await
}

#[derive(Debug)]
struct UserPool {
  matching: Deque<(u128, Waker)>,
  messages: HashMap<u128, (u128, String, Waker)>,
}

async fn chat(manual_stream: ManualStream<(), ServerStream, ()>) -> wtx::Result<()> {
  let wos = WebSocketOverStream::new(
    &Headers::new(),
    false,
    Xorshift64::from(simple_seed()),
    manual_stream.stream,
  )
  .await?;
  ws::exchange_messages(wos).await?;
  Ok(())
}
