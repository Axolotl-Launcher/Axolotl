use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use tokio::task::JoinHandle;
use tracing::warn;

#[derive(Default)]
pub(super) struct MinecraftLanAnnouncer {
    active: Option<ActiveAnnouncement>,
}

struct ActiveAnnouncement {
    port: u16,
    task: JoinHandle<()>,
}

impl MinecraftLanAnnouncer {
    pub(super) fn sync(&mut self, port: Option<u16>) {
        if self.active.as_ref().is_some_and(|active| {
            Some(active.port) == port && !active.task.is_finished()
        }) {
            return;
        }

        if let Some(active) = self.active.take() {
            active.task.abort();
        }
        if let Some(port) = port {
            self.active = Some(ActiveAnnouncement {
                port,
                task: tokio::spawn(run(port)),
            });
        }
    }
}

impl Drop for MinecraftLanAnnouncer {
    fn drop(&mut self) {
        if let Some(active) = self.active.take() {
            active.task.abort();
        }
    }
}

fn sockets() -> Vec<(Ipv4Addr, Socket)> {
    let mut addresses = local_ip_address::list_afinet_netifas()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(_, address)| match address {
            IpAddr::V4(address) if !address.is_unspecified() => Some(address),
            _ => None,
        })
        .collect::<Vec<_>>();
    addresses.sort_unstable();
    addresses.dedup();

    addresses
        .into_iter()
        .filter_map(|address| {
            let socket = (|| -> std::io::Result<Socket> {
                let socket = Socket::new(
                    Domain::IPV4,
                    Type::DGRAM,
                    Some(Protocol::UDP),
                )?;
                socket.set_multicast_if_v4(&address)?;
                socket.set_multicast_loop_v4(true)?;
                socket.set_multicast_ttl_v4(4)?;
                socket.bind(&SockAddr::from(SocketAddrV4::new(
                    Ipv4Addr::UNSPECIFIED,
                    0,
                )))?;
                Ok(socket)
            })();

            match socket {
                Ok(socket) => Some((address, socket)),
                Err(error) => {
                    warn!(
                        "failed to create Minecraft LAN announcer for {address}: {error}"
                    );
                    None
                }
            }
        })
        .collect()
}

async fn run(port: u16) {
    let sockets = sockets();
    if sockets.is_empty() {
        warn!("no IPv4 interfaces available for Minecraft LAN announcements");
        return;
    }

    let target =
        SockAddr::from(SocketAddrV4::new(Ipv4Addr::new(224, 0, 2, 60), 4445));
    let message =
        format!("[MOTD]Terracotta | Axolotl Multiplayer[/MOTD][AD]{port}[/AD]");
    let mut interval =
        tokio::time::interval(std::time::Duration::from_millis(1500));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        interval.tick().await;
        let mut sent = 0;
        for (address, socket) in &sockets {
            match socket.send_to(message.as_bytes(), &target) {
                Ok(_) => sent += 1,
                Err(error) => tracing::debug!(
                    target: "terracotta",
                    interface = %address,
                    "failed to send Minecraft LAN announcement: {error}"
                ),
            }
        }

        if sent == 0 {
            warn!(
                "failed to send Minecraft LAN announcement on every interface"
            );
            return;
        }
    }
}
