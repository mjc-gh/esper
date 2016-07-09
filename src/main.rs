#![feature(box_syntax, box_patterns)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::io::ErrorKind::{WouldBlock as BlockingErr};
use std::thread;
use std::sync::{Arc, Mutex};

extern crate hyper;
extern crate uuid;

use hyper::{Get, Post, StatusCode, RequestUri, Decoder, Encoder, Error, Control, Next};
use hyper::header::{ContentLength, ContentType};
use hyper::mime::{Mime, TopLevel, SubLevel};
use hyper::net::{HttpListener, HttpStream};
use hyper::server::{Server, Handler, Request, Response};

use uuid::Uuid;

static NOT_FOUND: &'static [u8] = b"404 Not Found";

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Client {
    id: Uuid
}

impl Client {
    fn new() -> Client {
        Client {
            id: Uuid::new_v4()
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct Topic {
    id: Option<Box<str>>
}

impl Topic {
    fn empty() -> Topic {
        Topic {
            id: None
        }
    }

    fn validate(skip: usize, full_path: &String) -> Option<Topic> {
        let id: String = full_path.chars().skip(skip).collect();

        match id.len() {
            0...64 => Some(Topic {
                id: Some(id.to_lowercase().into_boxed_str())
            }),

            _ => None
        }
    }
}

struct Message {
    body: Vec<u8>
}

impl Message {
    fn new(buf: &Vec<u8>) -> Message {
        // Append Event Source delimiter ("\n\n")
        let mut delimiter = vec![10 as u8, 10 as u8];
        let mut delimited_body = buf.clone();

        delimited_body.append(&mut delimiter);

        Message {
            body: delimited_body
        }
    }

    fn as_slice(&self) -> &[u8] {
        self.body.as_slice()
    }
}

struct Manager {
    messages: HashMap<Client, Vec<Message>>,
    streams: HashMap<Topic, Vec<(Client, Control)>>
}

impl Manager {
    fn new() -> Manager {
        Manager {
            messages: HashMap::new(),
            streams: HashMap::new()
        }
    }

    fn subscribe(&mut self, client: Client, topic: Topic, ctrl: Control) -> () {
        // Create client's message queue
        self.messages.insert(client.clone(), Vec::new());

        // Now "subscribe" the Client and Control stream to the Topic
        self.streams.entry(topic).or_insert(Vec::new()).push((client, ctrl));
    }

    fn unsubscribe(&mut self, client: Client, topic: Topic) -> () {
        // Remove the message queue
        self.messages.remove(&client.clone());

        // Remove the "subscribed" Client and Control tuple by index
        match self.streams.get_mut(&topic) {
            Some(mut list) => {
                match list.binary_search_by(|tuple| tuple.0.cmp(&client)) {
                    Ok(index) => {
                        list.remove(index);
                    }

                    Err(_) => ()
                }
            }

            None => ()
        }
    }

    fn publish(&mut self, topic: Topic, msg: &Vec<u8>){
        // Enumerate each client control tuple
        match self.streams.get(&topic) {
            Some(list) => {
                for &(ref client, ref ctrl) in list {
                    // Add message to client's queue
                    self.messages.get_mut(&client.clone()).unwrap().push(Message::new(msg));

                    // Signal Control to wakeup
                    ctrl.ready(Next::write()).unwrap();
                }
            }

            None => ()
        }
    }

    fn messages_for(&mut self, client: Client) -> Option<&mut Vec<Message>> {
        self.messages.get_mut(&client)
    }

    //fn topic_len(&self) -> usize {
    //self.streams.len()
    //}

    //fn client_len(&self) -> usize {
    //self.messages.len()
    //}
}

enum Route {
    NotFound,
    Subscribe,
    Publish(Body)
}

#[derive(Clone, Copy)]
enum Body {
    Len(u64),
    Chunked
}

struct EventStream {
    id: Client,
    msg_buf: Vec<u8>,
    msg_pos: usize,
    route: Route,
    topic: Topic,
    control: Control,
    manager: Arc<Mutex<Manager>>
}

impl EventStream {
    fn new(ctrl: Control, mgr: Arc<Mutex<Manager>>) -> EventStream {
        EventStream {
            id: Client::new(),
            msg_buf: vec![0; 4096],
            msg_pos: 0,
            topic: Topic::empty(),
            route: Route::NotFound,
            control: ctrl,
            manager: mgr
        }
    }
}

impl Handler<HttpStream> for EventStream {
    fn on_request(&mut self, request: Request<HttpStream>) -> Next {
        match *request.uri() {
            RequestUri::AbsolutePath(ref path) => match request.method() {
                &Get => {
                    if path.starts_with("/subscribe/") {
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
                match self.manager.lock() {
                    Ok(mut mgr) => {
                        let tpc = self.topic.clone();

                        mgr.publish(tpc, &self.msg_buf);
                    }

                    Err(_) => ()
                }

                Next::end()
            }

            Route::Subscribe => {
                response.headers_mut().set(ContentType(Mime(TopLevel::Text, SubLevel::EventStream, vec![])));

                match self.manager.lock() {
                    Ok(mut mgr) => {
                        let cid = self.id.clone();
                        let tpc = self.topic.clone();

                        mgr.subscribe(cid, tpc, self.control.clone());

                        Next::wait()
                    }

                    Err(_) => Next::end()
                }
            }

            Route::NotFound => {
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
                                        Ok(_) => (),
                                        Err(_) => {
                                            //mgr.unsubscribe(self.id.clone(), self.topic.clone());

                                            return Next::end()
                                        }
                                    }
                                }

                                msgs.clear();
                            }

                            None => ()
                        }

                        Next::wait()
                    }

                    Err(_) => Next::end()
                }
            }

            Route::NotFound => {
                transport.write(NOT_FOUND).unwrap();

                Next::end()
            }

            _ => unreachable!()
        }
    }

    fn on_error(&mut self, _err: Error) -> Next {
        println!("ON_ERR");

        match self.manager.lock() {
            Ok(mut mgr) => {
                mgr.unsubscribe(self.id.clone(), self.topic.clone());
            }

            Err(_) => println!("Failed to lock manager")
        }

        Next::end()
    }

    fn on_remove(self, _transport: HttpStream) -> () {
        println!("ON_REMOVE");

        match self.manager.lock() {
            Ok(mut mgr) => {
                mgr.unsubscribe(self.id.clone(), self.topic.clone());
            }

            Err(_) => println!("Failed to lock manager")
        }
    }
}

fn main() {
    let listener = HttpListener::bind(&"127.0.0.1:3002".parse().unwrap()).unwrap();
    let mut handles = Vec::new();

    let mgr: Manager = Manager::new();
    let mgr_ref = Arc::new(Mutex::new(mgr));

    for _ in 0..1 { // TODO CLI argument for number of server threads
        let listener = listener.try_clone().unwrap();
        let mgr_inner = mgr_ref.clone();

        handles.push(thread::spawn(move || {
            Server::new(listener).handle(|ctrl| {
                EventStream::new(ctrl, mgr_inner.clone())
            }).unwrap();
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
