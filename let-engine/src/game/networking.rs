use std::{marker::PhantomData, str::FromStr};

use ahash::HashMap;
use anyhow::{anyhow, Result};
use async_std::{
    io::{ReadExt, WriteExt},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream, UdpSocket},
    sync::{Arc, Mutex},
    task,
};
use serde::{Deserialize, Serialize};

type Msgs = Arc<Mutex<Vec<(SocketAddr, Vec<u8>)>>>;

pub struct GameServer<Msg>
where
    for<'a> Msg: Send + Serialize + Deserialize<'a>,
{
    tcp_listener: TcpListener,
    udp_socket: UdpSocket,
    connections: HashMap<SocketAddr, TcpStream>,
    messages: Msgs,
    _msg: PhantomData<Msg>,
}

impl<Msg> GameServer<Msg>
where
    for<'a> Msg: Send + Serialize + Deserialize<'a>,
{
    /// Creates a new server using the given address.
    pub fn new(addr: SocketAddr) -> Result<Self> {
        task::block_on(async {
            let tcp_listener = TcpListener::bind(addr).await?;

            let udp_socket = UdpSocket::bind(addr).await?;

            Ok(Self {
                tcp_listener,
                udp_socket,
                connections: HashMap::default(),
                messages: Arc::new(Mutex::new(vec![])),
                _msg: PhantomData,
            })
        })
    }

    /// Creates a new server only accessable on this machine with the given port.
    pub fn new_local(port: u16) -> Result<Self> {
        let addr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), port);
        Self::new(addr.into())
    }

    /// Creates a new server accessable by every device trying to connect with the given port.
    pub fn new_public(port: u16) -> Result<Self> {
        let addr = local_ip_addr::get_local_ip_address()?;
        let addr = SocketAddr::new(Ipv4Addr::from_str(&addr)?.into(), port);
        Self::new(addr)
    }

    pub(crate) async fn accept_connetions(&mut self) {
        while let Ok((stream, addr)) = self.tcp_listener.accept().await {
            let stream2 = stream.clone();
            let messages = self.messages.clone();
            task::spawn(recv_messages(stream2, addr, messages));
            self.connections.insert(addr, stream);
        }
    }
}

async fn recv_messages(mut stream: TcpStream, addr: SocketAddr, messages: Msgs) {
    let messages = messages;
    loop {
        let mut buf = [0u8; 1024];
        let size = stream.read(&mut buf).await.unwrap();

        messages.lock().await.push((addr, buf[..size].to_vec()));
    }
}

impl<Msg> GameServer<Msg>
where
    for<'a> Msg: Send + Serialize + Deserialize<'a>,
{
    /// Broadcasts a message to every client through TCP.
    ///
    /// This function should be used to broadcast important messages.
    pub async fn broadcast(&mut self, message: &Msg) -> Result<()> {
        for connection in self.connections.values_mut() {
            connection.write_all(&bincode::serialize(message)?).await?;
        }
        Ok(())
    }

    /// Sends a message to a specific target through TCP.
    ///
    /// This function should be used to send important messages.
    pub async fn send(&mut self, receiver: SocketAddr, message: &Msg) -> Result<()> {
        self.connections
            .get_mut(&receiver)
            .ok_or(anyhow!("Receiver does not exist"))?
            .write_all(&bincode::serialize(message)?)
            .await?;
        Ok(())
    }

    /// Broadcasts a message to every client through UDP.
    ///
    /// This function should be used to broadcast messages with the lowest latency possible.
    pub async fn udp_broadcast(&mut self, message: &Msg) -> Result<()> {
        for connection in self.connections.keys() {
            self.udp_socket
                .send_to(&bincode::serialize(message)?, connection)
                .await?;
        }
        Ok(())
    }

    /// Sends a message to a specific target through UDP.
    ///
    /// This function should be used to send messages with the lowest latency possible.
    pub async fn udp_send(&mut self, receiver: SocketAddr, message: &Msg) -> Result<()> {
        self.udp_socket
            .send_to(&bincode::serialize(message)?, receiver)
            .await?;
        Ok(())
    }

    pub async fn receive_messages(&mut self) -> Result<Vec<(SocketAddr, Msg)>> {
        let mut messages: Vec<(SocketAddr, Msg)> = vec![];
        for message in self.messages.lock().await.iter() {
            messages.push((message.0, bincode::deserialize(&message.1)?));
        }
        Ok(messages)
    }
}
