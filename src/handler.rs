use {Manager, Client, Topic};

use std::io::{Read, Write};
use std::io::ErrorKind::{WouldBlock as BlockingErr};
use std::sync::{Arc, Mutex};

use hyper::{Get, Post, StatusCode, RequestUri, Decoder, Encoder, Error, Control, Next};
use hyper::header::{ContentLength, ContentType};
use hyper::mime::{Mime, TopLevel, SubLevel};
use hyper::net::HttpStream;
use hyper::server::{Handler, Request, Response};

static NOT_FOUND: &'static [u8] = b"404 Not Found";

enum Route {
    NotFound,
    Subscribe,
    Publish(Body),
    Stats
}

#[derive(Clone, Copy)]
enum Body {
    Len(u64),
    Chunked
}

pub struct EventStream {
    id: Client,
    msg_buf: Vec<u8>,
    msg_pos: usize,
    route: Route,
    topic: Topic,
    control: Control,
    manager: Arc<Mutex<Manager>>,
    response: Option<String>
}

impl EventStream {
    pub fn new(ctrl: Control, mgr: Arc<Mutex<Manager>>) -> EventStream {
        EventStream {
            id: Client::new(),
            msg_buf: vec![0; 4096],
            msg_pos: 0,
            topic: Topic::empty(),
            route: Route::NotFound,
            control: ctrl,
            manager: mgr,
            response: None
        }
    }
}

impl Handler<HttpStream> for EventStream {
    fn on_request(&mut self, request: Request<HttpStream>) -> Next {
        info!("{} {}", request.method(), request.uri());

        match *request.uri() {
            RequestUri::AbsolutePath(ref path) => match request.method() {
                &Get => {
                    if path == "/stats" {
                        self.route = Route::Stats

                    } else if path.starts_with("/subscribe/") {
                        match Topic::validate(11, path) {
                            Some(topic) => {
                                self.topic = topic;
                                self.route = Route::Subscribe;
                            }

                            None => unreachable!()
                        }
                    }

                    Next::write()
                }

                &Post => {
                    if path.starts_with("/publish/") {
                        match Topic::validate(9, path) {
                            Some(topic) => {
                                let mut body_left = true;
                                let body = if let Some(len) = request.headers().get::<ContentLength>() {
                                    body_left = **len > 0;

                                    Body::Len(**len)
                                } else {
                                    Body::Chunked
                                };

                                self.topic = topic;
                                self.route = Route::Publish(body);

                                if body_left {
                                    return Next::read_and_write();
                                }
                            }

                            None => unreachable!()
                        }
                    }

                    Next::write()
                }

                _ => Next::write()
            },

            _ => Next::write()
        }
    }

    fn on_request_readable(&mut self, transport: &mut Decoder<HttpStream>) -> Next {
        match self.route {
            Route::Publish(ref body) => {
                debug!("POST /publish req_readable");

                if self.msg_pos < self.msg_buf.len() {
                    match transport.read(&mut self.msg_buf[self.msg_pos..]) {
                        Ok(n) => {
                            self.msg_pos += n;

                            match *body {
                                Body::Len(max) if max > self.msg_pos as u64 => {
                                    Next::read_and_write()
                                }

                                _ => Next::write()
                            }
                        }

                        Err(e) => match e.kind() {
                            BlockingErr => Next::read_and_write(),

                            _ => Next::end()
                        }
                    }
                } else {
                    Next::write()
                }
            }

            _ => unreachable!()
        }
    }

    fn on_response(&mut self, response: &mut Response) -> Next {
        match self.route {
            Route::Publish(_) => {
                debug!("POST /publish on_response");

                match self.manager.lock() {
                    Ok(mut mgr) => {
                        mgr.publish(self.topic.clone(), &self.msg_buf);
                    }

                    Err(_) => warn!("Failed to lock manager")
                }

                Next::end()
            }

            Route::Subscribe => {
                debug!("GET /subscribe on_response");

                response.headers_mut().set(ContentType(Mime(TopLevel::Text, SubLevel::EventStream, vec![])));

                match self.manager.lock() {
                    Ok(mut mgr) => {
                        mgr.subscribe(self.id.clone(), self.topic.clone(), self.control.clone());

                        Next::wait()
                    }

                    Err(_) => {
                        warn!("Failed to lock manager!");

                        Next::end()
                    }
                }
            }

            Route::Stats => {
                debug!("GET /stats on_response");

                match self.manager.lock() {
                    Ok(mgr) => {
                        match mgr.stats_json() {
                            Ok(json) => {
                                response.headers_mut().set(ContentLength(json.len() as u64));

                                self.response = Some(json);

                                Next::write()
                            }

                            Err(e) => {
                                warn!("JSON Error; err={:?}", e);

                                Next::end()
                            }
                        }
                    }

                    Err(_) => {
                        warn!("Failed to lock manager!");

                        Next::end()
                    }
                }
            }

            Route::NotFound => {
                debug!("Route Not Found on_response");

                response.set_status(StatusCode::NotFound);
                response.headers_mut().set(ContentLength(NOT_FOUND.len() as u64));

                Next::write()
            }
        }
    }

    fn on_response_writable(&mut self, transport: &mut Encoder<HttpStream>) -> Next {
        match self.route {
            Route::Subscribe => {
                match self.manager.lock() {
                    Ok(mut mgr) => {
                        match mgr.messages_for(self.id.clone()) {
                            Some(mut msgs) => {
                                for msg in msgs.iter() {
                                    match transport.write(msg.as_slice()) {
                                        Ok(_) => debug!("Transport wrote message"),
                                        Err(e) => {
                                            warn!("Transport IO Error; err={:?}", e);

                                            return Next::end()
                                        }
                                    }
                                }

                                msgs.clear();

                                debug!("Messages queue cleared");
                            }

                            None => warn!("No messages found for awoken handler")
                        }

                        Next::wait()
                    }

                    Err(_) => Next::end()
                }
            }

            Route::Stats => {
                match self.response {
                    Some(ref json) => {
                        transport.write(json.as_bytes()).unwrap();
                    }

                    None => ()
                }

                Next::end()
            }

            Route::NotFound => {
                transport.write(NOT_FOUND).unwrap();

                Next::end()
            }

            _ => unreachable!()
        }
    }

    fn on_error(&mut self, _err: Error) -> Next {
        match self.manager.lock() {
            Ok(mut mgr) => {
                mgr.unsubscribe(self.id.clone(), self.topic.clone());
            }

            Err(_) => warn!("Failed to lock manager")
        }

        Next::end()
    }

    fn on_remove(self, _transport: HttpStream) -> () {
        match self.manager.lock() {
            Ok(mut mgr) => {
                mgr.unsubscribe(self.id.clone(), self.topic.clone());
            }

            Err(_) => warn!("Failed to lock manager")
        }
    }
}

