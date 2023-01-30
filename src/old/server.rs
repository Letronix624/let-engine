extern crate lazy_static;
use crate::data::Data;

use super::Object;
use local_ip_address::local_ip;
use std::collections::HashMap;
use std::io::{ErrorKind, Read, Result, Write};
use std::iter::zip;
use std::net::{IpAddr, SocketAddr, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;

lazy_static::lazy_static! {
    static ref CONNECTIONS: Mutex<HashMap<SocketAddr, TcpStream>> = Mutex::new(HashMap::new());
    static ref PLAYERDATA: Mutex<HashMap<SocketAddr, Object>> = Mutex::new(HashMap::new());
    static ref UDP: Arc<Mutex<Option<UdpSocket>>> = Arc::new(Mutex::new(None));
}

#[derive(Clone)]
pub struct Server {
    pub ip: IpAddr,
    pub port: usize,
}

impl Server {
    pub fn init() -> Result<Server> {
        let ip: IpAddr;
        let port = 7777;
        match local_ip() {
            Ok(t) => ip = t,
            Err(e) => {
                return Err(std::io::Error::new(ErrorKind::Other, e));
            }
        };

        Ok(Self { ip, port })
    }
    pub fn start(&mut self) -> Result<()> {
        let udpserver = UdpSocket::bind(format!("{}:{}", &self.ip, &self.port))?;
        let udpserver2 = udpserver.try_clone()?;
        thread::spawn(move || Self::receiveudp(udpserver));

        let _ = UDP.lock().unwrap().insert(udpserver2);

        println!("Server started on {}:{}.", &self.ip, &self.port);
        Ok(())
    }
    pub fn tcpconnection(conn: TcpStream, addr: SocketAddr) {
        let connection = || -> Result<()> {
            let mut receiver: TcpStream = conn.try_clone()?;
            let mut buf = [0; 1024];
            receiver.read(&mut buf)?;
            if buf.starts_with(&"!~let login".as_bytes()) {
                // Differentiate between some tcp packet senders and actual players.
                PLAYERDATA.lock().unwrap().insert(addr, Object::empty());
                let conn2 = conn.try_clone()?;
                receiver.write(&[1])?;
                let mut connections = CONNECTIONS.lock().unwrap();
                connections.insert(addr, conn2);
                println!("{:?} connected.", addr);
                drop(connections);
                loop {
                    let mut buf = [0u8; 1024];
                    let rsize = receiver.read(&mut buf)?;
                    if rsize != 0 {
                        match std::str::from_utf8(&buf) {
                            Ok(t) => {
                                println!("TCP - {addr}: {t}");
                            }
                            Err(_) => (),
                        };
                    } else {
                        break;
                    }
                }
                let mut connections = CONNECTIONS.lock().unwrap();
                connections.remove(&addr);
                drop(connections);
                PLAYERDATA.lock().unwrap().remove(&addr);
            }
            Ok(())
        };
        match connection() {
            Ok(_) => println!("{} disconnected.", addr),
            Err(e) => println!("{} disconnected. ({})", addr, e),
        };
    }
    fn receiveudp(server: UdpSocket) {
        loop {
            let mut buf = [0; 1024];
            let (bufs, addr) = server.recv_from(&mut buf).unwrap();
            if bufs > 0 && CONNECTIONS.lock().unwrap().contains_key(&addr) {
                match buf {
                    msg if msg.starts_with(&[5u8]) => match server.send_to(&[5u8], addr) {
                        Ok(_) => (),
                        Err(_) => break,
                    },
                    _ => match buf[0] {
                        1 => {
                            let obj = Object {
                                position: [
                                    f32::from_be_bytes(buf[1..5].try_into().unwrap()),
                                    f32::from_be_bytes(buf[5..9].try_into().unwrap()),
                                ],
                                size: [
                                    f32::from_be_bytes(buf[9..13].try_into().unwrap()),
                                    f32::from_be_bytes(buf[13..17].try_into().unwrap()),
                                ],
                                rotation: 0.0,
                                color: [1.0, 0.0, 1.0, 1.0],
                                texture: None,
                                data: Data::square(),
                                parent: None,
                            };
                            {
                                let mut pd = PLAYERDATA.lock().unwrap();
                                pd.insert(addr, obj);
                            }
                        }
                        _ => {
                            println!("{:?}", buf)
                        }
                    },
                }
            }
        }
    }
    #[allow(unused)]
    pub fn broadcasttcp(&self, buf: &[u8]) -> Result<()> {
        for mut conn in CONNECTIONS.lock().unwrap().values() {
            conn.write(&buf)?;
        }
        Ok(())
    }
    pub fn broadcastobjs(&self) -> Result<()> {
        match UDP.clone().lock().unwrap().as_ref() {
            None => (),
            Some(t) => {
                let mut buf = [0; 1024];
                for addr in CONNECTIONS.lock().unwrap().keys() {
                    // for every connection
                    let mut oid = 0;
                    for obj in zip(PLAYERDATA.lock().unwrap().iter(), 0..) {
                        // for every online game object
                        if obj.0 .0 != addr {
                            let object = obj.0 .1;

                            let mut namebuf: [u8; 15] = [0; 15];
                            let name = format!("player{}", obj.1 + 2);
                            namebuf[0..name.len()].copy_from_slice(name.as_bytes());
                            buf[0 + oid] = 1;
                            buf[1 + oid..5 + oid]
                                .copy_from_slice(&object.position[0].to_be_bytes());
                            buf[5 + oid..9 + oid]
                                .copy_from_slice(&object.position[1].to_be_bytes());
                            buf[9 + oid..13 + oid].copy_from_slice(&object.size[0].to_be_bytes());
                            buf[13 + oid..17 + oid].copy_from_slice(&object.size[1].to_be_bytes());
                            buf[17 + oid..32 + oid].copy_from_slice(&namebuf);
                            oid = oid + 32; // 0, 32, 64, 96
                        }
                    }
                    t.send_to(&buf, addr)?;
                }
            }
        }
        Ok(())
    }
    #[allow(unused)]
    pub fn broadcastudp(&self, buf: &[u8]) -> Result<()> {
        for conn in CONNECTIONS.lock().unwrap().keys() {
            match UDP.clone().lock().unwrap().as_ref() {
                None => (),
                Some(t) => {
                    t.send_to(&buf, conn)?;
                }
            }
        }
        Ok(())
    }
}
