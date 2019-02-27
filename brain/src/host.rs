use rand::thread_rng;
use tiny_http::{Method, Response, Server as HttpServer};

use crate::markov::Markov;

pub struct Server<'a> {
    server: HttpServer,
    markov: Markov<'a>,
}

impl<'a> Server<'a> {
    pub fn new(addr: &str, markov: Markov<'a>) -> Self {
        let server = HttpServer::http(addr).unwrap();
        println!("hosting at http://{}", addr);
        Self { server, markov }
    }

    pub fn start(&mut self) {
        let mut rng = thread_rng();

        // TODO use async/await for this
        for req in self.server.incoming_requests() {
            match (req.method(), req.url()) {
                (&Method::Get, "/markov/next") => {
                    timeit!("generate response");
                    let data = self.markov.generate(&mut rng);
                    let resp = Response::from_string(data);
                    let _ = req.respond(resp);
                }
                (_, _) => {
                    let resp = Response::from_string("404 not found");
                    let resp = resp.with_status_code(404);
                    let _ = req.respond(resp);
                }
            }
        }
    }
}
