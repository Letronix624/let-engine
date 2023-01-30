use crate::data::Data;
use crate::data::SQUARE_ID;

use super::SQUARE;

use super::Object;
use std::collections::HashMap;
use std::io::{Read, Result, Write};
use std::net::{TcpStream, UdpSocket};
use std::str;
use std::sync::Mutex;
use std::thread;
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static mut PINGTIMER: u128 = 0;
static mut PING: u32 = 0;

lazy_static::lazy_static! {
    static ref UDPSERVER: Mutex<Option<UdpSocket>> = Mutex::new(None);
    static ref TCPSERVER: Mutex<Option<TcpStream>> = Mutex::new(None);
    pub static ref GAMEOBJECTS: Mutex<HashMap<String, Object>> = Mutex::new(HashMap::new());
}

fn set_ping() {
    unsafe { PING = (unix_timestamp() - PINGTIMER) as u32 }
}
#[allow(dead_code)] //temp
pub fn get_ping() -> u32 {
    unsafe { PING }
}

#[derive(Clone)]
pub struct Client {
    pub ip: String,
    pub port: usize,
    pub connected: bool,
}
#[allow(dead_code)] //temp
impl Client {
    pub fn new() -> Self {
        Self {
            ip: "seflon.ddns.net".to_string(),
            port: 7777,
            connected: false,
        }
    }
    #[allow(dead_code)]
    pub fn from(ip: String, port: usize) -> Self {
        Self {
            ip,
            port,
            connected: false,
        }
    }
    pub fn connect(&mut self) -> Result<()> {
        let mut tcpserver: TcpStream = TcpStream::connect(format!("{}:{}", self.ip, self.port))?;

        let mut udpserver: UdpSocket = UdpSocket::bind(tcpserver.local_addr().unwrap())?;

        {
            let mut udp = UDPSERVER.lock().unwrap();
            let _yes = udp.insert(udpserver);
            udpserver = udp.as_ref().unwrap().try_clone().unwrap();
        }
        {
            let mut tcp = TCPSERVER.lock().unwrap();
            let _yes = tcp.insert(tcpserver);
            tcpserver = tcp.as_ref().unwrap().try_clone().unwrap();
        }

        udpserver.connect(format!("{}:{}", self.ip, self.port))?;

        let tcpserver2 = tcpserver.try_clone().unwrap();
        let mut tcpserver3 = tcpserver.try_clone().unwrap();
        tcpserver3.write(&"!~let login".as_bytes()).unwrap(); // "!~let login (password)" so the server knows if the connecting person is a player or just some random ip sweeper.
        let mut buf = [0; 1024];
        tcpserver3.read(&mut buf).unwrap();
        if buf.starts_with(&[0]) {
            // Server could be full or get a wrong password. in case of that you get kicked and getting kicked returns empty buffers.
            return Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                format!(
                    "Failed to connect to {}. Server refused to add you to the server.",
                    self.ip
                ),
            ));
        }
        self.connected = true;
        println!("Connected to {}!", self.ip);

        let udpserver2 = udpserver.try_clone().unwrap();

        let udpsender = thread::spawn(move || match Self::ping() {
            //send udp
            _ => (),
        });
        let tcpreceiver = thread::spawn(move || {
            match Self::receivetcp(tcpserver2) {
                _ => (),
            };
        });
        let udpreceiver = thread::spawn(move || {
            match Self::receiveudp(udpserver2) {
                _ => (),
            };
        });
        thread::spawn(move || {
            udpsender.join().unwrap();
            tcpreceiver.join().unwrap();
            udpreceiver.join().unwrap();
            println!("Disconnected from server.")
        });
        Ok(())
    }
    #[allow(dead_code)]
    fn chat(msg: &str) -> Result<()> {
        let serverlock = TCPSERVER.lock().unwrap();
        let mut server = serverlock.as_ref().unwrap();
        server.write(msg.as_bytes())?;
        sleep(Duration::new(1, 0));
        Ok(())
    }
    pub fn sendobject(&self, obj: Object) -> Result<()> {
        let mut buf = [0; 1024];
        buf[0] = 1;
        buf[1..5].copy_from_slice(&obj.position()[0].to_be_bytes());
        buf[5..9].copy_from_slice(&obj.position()[1].to_be_bytes());
        buf[9..13].copy_from_slice(&obj.size[0].to_be_bytes());
        buf[13..17].copy_from_slice(&obj.size[1].to_be_bytes());

        UDPSERVER.lock().unwrap().as_ref().unwrap().send(&buf)?;
        Ok(())
    }
    fn ping() -> Result<()> {
        loop {
            unsafe { PINGTIMER = unix_timestamp() }
            UDPSERVER.lock().unwrap().as_ref().unwrap().send(&[5u8])?;

            sleep(Duration::from_secs(5))
        }
    }
    fn receiveudp(server: UdpSocket) -> Result<()> {
        loop {
            let mut buf = [0; 1024];
            let bufsize = server.recv(&mut buf)?;
            if bufsize > 0 {
                match buf {
                    msg if msg.starts_with(&[5u8]) => set_ping(),
                    msg if msg.starts_with(&[1u8]) => {
                        let mut diter = buf.iter().enumerate();
                        let mut data = diter.next();
                        while data.is_some() && data.unwrap().1 != &0 {
                            //continure doing this, let let let let let let let let le tlet l.~!
                            let d = data.unwrap().1;

                            if d == &1 {
                                let c = data.unwrap().0;
                                for _ in 1..32 {
                                    diter.next();
                                }
                                data = diter.next();

                                let obj = Object {
                                    position: [
                                        f32::from_be_bytes(buf[1 + c..5 + c].try_into().unwrap()),
                                        f32::from_be_bytes(buf[5 + c..9 + c].try_into().unwrap()),
                                    ],
                                    size: [
                                        f32::from_be_bytes(buf[9 + c..13 + c].try_into().unwrap()),
                                        f32::from_be_bytes(buf[13 + c..17 + c].try_into().unwrap()),
                                    ],
                                    rotation: 0.0,
                                    color: [1.0, 0.0, 1.0, 1.0],
                                    texture: None,
                                    data: Data::square(),
                                    parent: None,
                                };
                                let objname = str::from_utf8(&buf[17 + c..32 + c]).unwrap();
                                GAMEOBJECTS.lock().unwrap().insert(objname.to_string(), obj);
                            } else {
                                break;
                            }
                        }
                    }
                    _ => (),
                }
            } else {
                break;
            }
        }
        Ok(())
    }
    fn receivetcp(mut server: TcpStream) -> Result<()> {
        loop {
            let mut buf = [0; 1024];
            let bufsize = server.read(&mut buf)?;
            if bufsize > 0 {
                println!("TCP - Server: {}", str::from_utf8(&buf).unwrap());
            } else {
                break;
            }
        }
        Ok(())
    }
}
fn unix_timestamp() -> u128 {
    return SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
}
