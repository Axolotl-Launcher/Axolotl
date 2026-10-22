use crate::state::FriendsSocket;
use crate::state::friends::{TunnelSockets, WriteSocket};
use ariadne::networking::message::ClientToServerMessage;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub(super) enum InternalTunnelSocket {
    Listening(SocketAddr),
    Connected {
        owner: Uuid,
        stream: Mutex<OwnedWriteHalf>,
        cancel: CancellationToken,
    },
}

pub(super) type SocketRegistry =
    parking_lot::Mutex<HashMap<Uuid, Arc<InternalTunnelSocket>>>;

impl InternalTunnelSocket {
    pub(super) fn register_connection(
        sockets: &TunnelSockets,
        owner: Uuid,
        listener: &Arc<Self>,
        socket_id: Uuid,
        stream: TcpStream,
    ) -> Option<(OwnedReadHalf, CancellationToken)> {
        let mut sockets = sockets.lock();
        // Registration and listener removal share one lock, including after
        // the asynchronous local TCP connect has completed.
        if !sockets.get(&owner).is_some_and(|current| {
            Arc::ptr_eq(current, listener)
                && matches!(**current, Self::Listening(_))
        }) {
            return None;
        }
        let (read, write) = stream.into_split();
        let cancel = CancellationToken::new();
        sockets.insert(
            socket_id,
            Arc::new(Self::Connected {
                owner,
                stream: Mutex::new(write),
                cancel: cancel.clone(),
            }),
        );
        Some((read, cancel))
    }
}

impl Drop for InternalTunnelSocket {
    fn drop(&mut self) {
        if let Self::Connected { cancel, .. } = self {
            cancel.cancel();
        }
    }
}

pub(super) fn take_sockets(
    sockets: &TunnelSockets,
    socket_id: Uuid,
) -> Vec<(Uuid, Arc<InternalTunnelSocket>)> {
    let mut removed = Vec::new();
    sockets.lock().retain(|id, socket| {
        let owned = matches!(**socket, InternalTunnelSocket::Connected { owner, .. } if owner == socket_id);
        if *id != socket_id && !owned {
            return true;
        }
        if let InternalTunnelSocket::Connected { cancel, .. } = &**socket {
            cancel.cancel();
        }
        removed.push((*id, socket.clone()));
        false
    });
    removed
}

pub(super) async fn close_sockets(
    write: WriteSocket,
    sockets: Vec<(Uuid, Arc<InternalTunnelSocket>)>,
) -> crate::Result<()> {
    let mut first_error = None;
    // Local cleanup must finish even if the control socket cannot send.
    for (_, socket) in &sockets {
        if let InternalTunnelSocket::Connected { stream, .. } = &**socket
            && let Err(error) = stream.lock().await.shutdown().await
        {
            first_error.get_or_insert_with(|| crate::Error::from(error));
        }
    }
    for (socket, _) in sockets {
        if let Err(error) = FriendsSocket::send_message(
            &write,
            ClientToServerMessage::SocketClose { socket },
        )
        .await
        {
            first_error.get_or_insert(error);
        }
    }
    first_error.map_or(Ok(()), Err)
}

pub struct TunnelSocket {
    pub(super) socket_id: Uuid,
    pub(super) write: WriteSocket,
    pub(super) sockets: TunnelSockets,
}

impl TunnelSocket {
    pub fn socket_id(&self) -> Uuid {
        self.socket_id
    }

    pub async fn shutdown(self) -> crate::Result<()> {
        let sockets = take_sockets(&self.sockets, self.socket_id);
        if sockets.is_empty() {
            return Ok(());
        }
        // Detach cleanup from the caller's future so cancellation cannot
        // discard sockets already removed from the registry.
        tokio::spawn(close_sockets(self.write.clone(), sockets)).await?
    }
}

impl Drop for TunnelSocket {
    fn drop(&mut self) {
        let sockets = take_sockets(&self.sockets, self.socket_id);
        if !sockets.is_empty() {
            tokio::spawn(close_sockets(self.write.clone(), sockets));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;
    use tokio::sync::RwLock;
    use tokio::time::{Duration, timeout};

    fn listener(sockets: &TunnelSockets, write: WriteSocket) -> TunnelSocket {
        let socket_id = Uuid::new_v4();
        sockets.lock().insert(
            socket_id,
            Arc::new(InternalTunnelSocket::Listening(
                "127.0.0.1:25565".parse().unwrap(),
            )),
        );
        TunnelSocket {
            socket_id,
            write,
            sockets: sockets.clone(),
        }
    }

    async fn tcp_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (client, server) = tokio::join!(
            TcpStream::connect(listener.local_addr().unwrap()),
            listener.accept(),
        );
        (client.unwrap(), server.unwrap().0)
    }

    async fn connect(
        tunnel: &TunnelSocket,
    ) -> (Uuid, TcpStream, CancellationToken) {
        let (client, peer) = tcp_pair().await;
        let id = Uuid::new_v4();
        let listening = tunnel
            .sockets
            .lock()
            .get(&tunnel.socket_id)
            .unwrap()
            .clone();
        let (read, cancel) = InternalTunnelSocket::register_connection(
            &tunnel.sockets,
            tunnel.socket_id,
            &listening,
            id,
            client,
        )
        .unwrap();
        FriendsSocket::socket_read_loop(
            tunnel.write.clone(),
            tunnel.sockets.clone(),
            read,
            id,
            cancel.clone(),
        );
        (id, peer, cancel)
    }

    async fn assert_closed(peer: &mut TcpStream) {
        let mut byte = [0];
        assert_eq!(
            timeout(Duration::from_secs(2), peer.read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn shutdown_closes_all_owned_connections_and_preserves_other_listeners()
     {
        let sockets = Arc::new(SocketRegistry::default());
        let write = Arc::new(RwLock::new(None));
        let tunnel = listener(&sockets, write.clone());
        let other = listener(&sockets, write);
        let (_, mut first, first_cancel) = connect(&tunnel).await;
        let (_, mut second, second_cancel) = connect(&tunnel).await;
        let (other_id, _other_peer, other_cancel) = connect(&other).await;
        tunnel.shutdown().await.unwrap();
        assert_eq!(sockets.lock().len(), 2);
        assert!(sockets.lock().contains_key(&other_id));
        assert!(first_cancel.is_cancelled());
        assert!(second_cancel.is_cancelled());
        assert!(!other_cancel.is_cancelled());
        assert_closed(&mut first).await;
        assert_closed(&mut second).await;
        other.shutdown().await.unwrap();
        assert!(sockets.lock().is_empty());
    }

    #[tokio::test]
    async fn drop_reclaims_connections_and_cancels_reads_without_peer_activity()
    {
        let sockets = Arc::new(SocketRegistry::default());
        let tunnel = listener(&sockets, Arc::new(RwLock::new(None)));
        let (_, mut peer, cancel) = connect(&tunnel).await;
        drop(tunnel);
        assert!(sockets.lock().is_empty());
        assert!(cancel.is_cancelled());
        assert_closed(&mut peer).await;
    }

    #[tokio::test]
    async fn a_local_connect_finishing_after_listener_removal_is_rejected() {
        let sockets = Arc::new(SocketRegistry::default());
        let tunnel = listener(&sockets, Arc::new(RwLock::new(None)));
        let owner = tunnel.socket_id;
        let listening = sockets.lock().get(&owner).unwrap().clone();
        let (stream, mut peer) = tcp_pair().await;
        tunnel.shutdown().await.unwrap();
        assert!(
            InternalTunnelSocket::register_connection(
                &sockets,
                owner,
                &listening,
                Uuid::new_v4(),
                stream,
            )
            .is_none()
        );
        assert!(sockets.lock().is_empty());
        assert_closed(&mut peer).await;
    }

    #[tokio::test]
    async fn local_eof_removes_the_connected_record() {
        let sockets = Arc::new(SocketRegistry::default());
        let tunnel = listener(&sockets, Arc::new(RwLock::new(None)));
        let (id, peer, cancel) = connect(&tunnel).await;
        drop(peer);
        timeout(Duration::from_secs(2), cancel.cancelled())
            .await
            .unwrap();
        assert!(!sockets.lock().contains_key(&id));
        assert!(sockets.lock().contains_key(&tunnel.socket_id));
        tunnel.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn local_cleanup_does_not_wait_for_the_control_socket() {
        let sockets = Arc::new(SocketRegistry::default());
        let write = Arc::new(RwLock::new(None));
        let tunnel = listener(&sockets, write.clone());
        let (_, mut peer, cancel) = connect(&tunnel).await;
        let control_lock = write.write().await;
        let shutdown = tokio::spawn(tunnel.shutdown());
        timeout(Duration::from_secs(2), cancel.cancelled())
            .await
            .unwrap();
        shutdown.abort();
        assert_closed(&mut peer).await;
        assert!(sockets.lock().is_empty());
        drop(control_lock);
    }

    #[tokio::test]
    async fn shutdown_notifies_the_server_once_for_listener_and_each_connection()
     {
        use async_tungstenite::tokio::{accept_async, connect_async};
        use async_tungstenite::tungstenite::Message;
        use either::Either;
        use futures::StreamExt;
        use std::collections::HashSet;

        let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = server.local_addr().unwrap();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let server_task = tokio::spawn(async move {
            let (stream, _) = server.accept().await.unwrap();
            let mut websocket = accept_async(stream).await.unwrap();
            let mut ids = Vec::new();
            while let Some(Ok(message)) = websocket.next().await {
                let parsed = match message {
                    Message::Text(text) => {
                        ClientToServerMessage::deserialize(Either::Left(&text))
                    }
                    Message::Binary(data) => {
                        ClientToServerMessage::deserialize(Either::Right(&data))
                    }
                    _ => continue,
                }
                .unwrap();
                if let ClientToServerMessage::SocketClose { socket } = parsed {
                    ids.push(socket);
                    if ids.len() == 3 {
                        break;
                    }
                }
            }
            sender.send(ids).unwrap();
        });
        let (websocket, _) =
            connect_async(format!("ws://{address}")).await.unwrap();
        let (sender, _) = websocket.split();
        let sockets = Arc::new(SocketRegistry::default());
        let tunnel = listener(&sockets, Arc::new(RwLock::new(Some(sender))));
        let owner = tunnel.socket_id;
        let (first, mut first_peer, _) = connect(&tunnel).await;
        let (second, mut second_peer, _) = connect(&tunnel).await;
        tunnel.shutdown().await.unwrap();
        assert_closed(&mut first_peer).await;
        assert_closed(&mut second_peer).await;
        let ids = timeout(Duration::from_secs(2), receiver)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ids.len(), 3);
        assert_eq!(
            ids.into_iter().collect::<HashSet<_>>(),
            HashSet::from([owner, first, second])
        );
        server_task.await.unwrap();
    }
}
