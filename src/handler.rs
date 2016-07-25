use {Access, Manager, Client, Topic};

use std::io::{Read, Write};
use std::io::ErrorKind::{WouldBlock as BlockingErr};
use std::sync::{Arc, Mutex};

use hyper::{Get, Post, StatusCode, RequestUri, Decoder, Encoder, Error, Control, Next};
use hyper::header::{ContentLength, ContentType};
use hyper::mime::{Mime, TopLevel, SubLevel};
use hyper::net::HttpStream;
use hyper::server::{Handler, Request, Response};
use url::form_urlencoded::{parse as parse_query_string};

static NOT_FOUND: &'static [u8] = b"404 Not Found";

enum Route {
    NotFound,
    Publish(Body),
    Stats,
    Subscribe,
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
    out_buf: Vec<u8>,
    route: Route,
    topic: Topic,
    control: Control,
    access: Arc<Access>,
    manager: Arc<Mutex<Manager>>
}

impl EventStream {
    pub fn new(ctrl: Control, acc: Arc<Access>, mgr: Arc<Mutex<Manager>>) -> EventStream {
        EventStream {
            id: Client::new(),
            msg_buf: vec![0; 4096],
            msg_pos: 0,
            out_buf: vec![0; 0],
            topic: Topic::new(),
            route: Route::NotFound,
            control: ctrl,
            access: acc,
            manager: mgr
        }
    }
}

fn split_absolute_path(abs_path: String) -> (String, Option<String>) {
    let split:Vec<&str> = abs_path.split("?").collect();

    match split.len() {
        2 => {
            match (split.first(), split.last()) {
                (Some(path), Some(query)) => {
                    (path.to_string(), Some(query.to_string()))
                }

                _ => (abs_path.clone(), None)
            }
        }

        _ => (abs_path.clone(), None)
    }
}

fn parse_and_find_token(query_string: Option<String>) -> Option<String> {
    match query_string {
        Some(qstr) => {
            let mut parsed = parse_query_string(qstr.as_bytes());

            match parsed.find(|ref tuple| tuple.0 == "token") {
                Some(pair) => Some(pair.1.into_owned()),
                None => None
            }
        }

        None => None
    }
}

impl Handler<HttpStream> for EventStream {
    fn on_request(&mut self, request: Request<HttpStream>) -> Next {
        info!("{} {}", request.method(), request.uri());

        match *request.uri() {
            RequestUri::AbsolutePath(ref abs_path) => {
                let (path, query_string) = split_absolute_path(abs_path.clone());
                let token = parse_and_find_token(query_string);

                    debug!("Found JWT parameter of {:?}", token);

                match request.method() {
                    &Get if path == "/stats" => {
                        debug!("Processing /stats requests");

                        if self.access.is_authenticated_for_publish(self.topic.id.clone(), token) {
                            self.route = Route::Stats;
                        }

                        Next::write()
                    }

                    &Get if path.starts_with("/subscribe") => {
                        if self.access.is_authenticated_for_subscribe(self.topic.id.clone(), token) {
                            match Topic::validate(11, path) {
                                Some(topic) => {
                                    self.topic = topic;
                                    self.route = Route::Subscribe;
                                }

                                None => ()
                            }
                        }

                        Next::write()
                    }

                    &Post if path.starts_with("/publish") => {
                        if self.access.is_authenticated_for_publish(self.topic.id.clone(), token) {
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

                                None => ()
                            }
                        }

                        Next::write()
                    }

                    _ => Next::write()
                }
            }

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

                                self.out_buf = json.into_bytes();

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
                        let msgs = mgr.messages_for(self.id.clone());

                        for msg in msgs.iter() {
                            match transport.write(msg.as_slice()) {
                                Ok(_) => debug!("Transport wrote message"),
                                Err(e) => {
                                    warn!("Transport IO Error; err={:?}", e);

                                    return Next::end()
                                }
                            }
                        }

                        Next::wait()
                    }

                    Err(_) => Next::end()
                }
            }

            Route::Stats => {
                if self.out_buf.len() > 0 {
                    transport.write(self.out_buf.as_slice()).unwrap();
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

