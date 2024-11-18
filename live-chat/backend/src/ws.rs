use crate::{ServerStream, USER_POOL};
use std::{
  future::{poll_fn, Future},
  mem,
  pin::pin,
  task::{ready, Poll},
};
use wtx::{
  http2::WebSocketOverStream,
  misc::{GenericTime, Vector, NOOP_WAKER},
  web_socket::{Frame, OpCode},
};

pub(crate) async fn exchange_messages(
  mut wos: WebSocketOverStream<ServerStream>,
) -> wtx::Result<()> {
  let local_id = GenericTime::timestamp()?.as_nanos();
  let remote_id = handshake(local_id, &mut wos).await?;
  let rslt = connection((local_id, remote_id), &mut wos).await;
  wos.close().await?;
  let mut user_guard = USER_POOL.lock().await;
  drop(user_guard.messages.remove(&local_id));
  if let Some((_, _, waker)) = user_guard.messages.remove(&remote_id) {
    waker.wake();
  }
  drop(user_guard);
  rslt
}

async fn connection(
  (local_id, remote_id): (u128, u128),
  wos: &mut WebSocketOverStream<ServerStream>,
) -> wtx::Result<()> {
  let mut buffer = Vector::new();
  loop {
    buffer.clear();
    let mut user_pin = pin!(USER_POOL.lock());
    let message_fut = poll_fn(|cx| {
      let mut user_guard = ready!(user_pin.as_mut().poll(cx));
      user_pin.set(USER_POOL.lock());
      let Some((_, message, waker)) = user_guard.messages.get_mut(&local_id) else {
        return Poll::Ready(Err(wtx::Error::ClosedConnection));
      };
      if message.is_empty() {
        waker.clone_from(cx.waker());
        return Poll::Pending;
      }
      Poll::Ready(wtx::Result::Ok(mem::take(message)))
    });
    tokio::select! {
      frame_rslt = wos.read_frame(&mut buffer) => {
        let frame = frame_rslt?;
        match frame.op_code() {
          OpCode::Text => {
            let Some(text) = frame.text_payload() else {
              return Err(wtx::web_socket::WebSocketError::UnexpectedFrame.into());
            };
            let mut user_guard = USER_POOL.lock().await;
            let Some((_, message, waker)) = user_guard.messages.get_mut(&remote_id) else {
              return Err(wtx::Error::ClosedConnection);
            };
            message.push_str(text);
            waker.wake_by_ref();
          }
          OpCode::Close => break,
          _ => {}
        }
      }
      message_rslt = message_fut => {
        wos.write_frame(&mut Frame::new_fin(OpCode::Text, message_rslt?.into_bytes())).await?;
      }
    }
  }
  Ok(())
}

async fn handshake(
  local_id: u128,
  wos: &mut WebSocketOverStream<ServerStream>,
) -> wtx::Result<u128> {
  let mut user_pin = pin!(USER_POOL.lock());
  let remote_id = poll_fn(|cx| {
    let mut user_guard = ready!(user_pin.as_mut().poll(cx));
    user_pin.set(USER_POOL.lock());
    if let Some((remote_id, _, _)) = user_guard.messages.get(&local_id) {
      return Poll::Ready(Ok(*remote_id));
    }
    if let Some((remote_id, remote_waker)) = user_guard.matching.pop_front() {
      drop(user_guard.messages.insert(local_id, (remote_id, String::new(), NOOP_WAKER.clone())));
      drop(user_guard.messages.insert(remote_id, (local_id, String::new(), NOOP_WAKER.clone())));
      remote_waker.wake();
      Poll::Ready(wtx::Result::Ok(remote_id))
    } else {
      user_guard.matching.push_back((local_id, cx.waker().clone()))?;
      Poll::Pending
    }
  })
  .await?;
  wos.write_frame(&mut Frame::new_fin(OpCode::Text, *b"OK")).await?;
  Ok(remote_id)
}
