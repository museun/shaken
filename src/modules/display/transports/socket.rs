use super::*;
use crate::queue::Queue;
use crate::util::get_timestamp;

use std::io::{self, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;

use crossbeam_channel as channel;

const MAX_MESSAGES: usize = 16;

pub struct SocketTransport {
    tx: channel::Sender<Message>,
    rx: channel::Receiver<Message>,
}

impl SocketTransport {
    pub fn new() -> Self {
        let (tx, rx) = channel::bounded(MAX_MESSAGES);
        Self::run_loop(rx.clone());
        Self { tx, rx }
    }
}

impl Transport for SocketTransport {
    fn send(&self, msg: Message) {
        if self.rx.is_full() {
            self.rx.recv();
        }
        self.tx.send(msg);
    }
}

impl SocketTransport {
    fn run_loop(rx: channel::Receiver<Message>) {
        struct Client {
            id: u64,
            last: u64,
            stream: TcpStream,
        }

        thread::spawn(move || {
            let mut queue = Queue::new(MAX_MESSAGES);
            let (mut clients, mut alive) = (vec![], vec![]);

            // TODO make port configurable, or automatic
            let listener = TcpListener::bind("localhost:51001").expect("listen on 51001");
            listener
                .set_nonblocking(true)
                .expect("listener must be non-blocking");

            info!(
                "display socket transport listening on: {}",
                listener.local_addr().unwrap()
            );

            // TODO figure out how to exit from this (XXX: why, though?)
            loop {
                loop {
                    match listener.accept() {
                        Ok((stream, addr)) => {
                            let client = Client {
                                id: clients.len() as u64,
                                last: 0,
                                stream,
                            };
                            debug!("accepted client: id:{} from {}", client.id, addr);
                            clients.push(client);
                            break;
                        }
                        Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {}
                        Err(err) => info!("could not accept client: {}", err),
                    }

                    if let Some(msg) = rx.try_recv() {
                        let ts = msg.timestamp;
                        let msg = serde_json::to_string(&msg).expect("valid json") + "\n";
                        queue.push((ts, msg));
                        break;
                    }

                    thread::park_timeout(std::time::Duration::from_millis(150));
                }

                'drain: for client in clients.drain(..) {
                    let mut client = client;
                    let last = client.last;
                    for (_, msg) in queue.iter().filter(|(ts, _)| *ts > last) {
                        if let Err(_err) = client.stream.write_all(msg.as_bytes()) {
                            let _ = client.stream.shutdown(Shutdown::Both);
                            trace!("{} write err", client.id);
                            continue 'drain;
                        }
                    }
                    if let Err(_err) = client.stream.flush() {
                        let _ = client.stream.shutdown(Shutdown::Both);
                        trace!("{} flush err", client.id);
                        continue;
                    }

                    client.last = get_timestamp();
                    alive.push(client)
                }

                std::mem::swap(&mut clients, &mut alive);
                clients.shrink_to_fit();
            }
        });
    }
}
