#![feature(plugin)]
#![plugin(docopt_macros)]

use std::collections::HashMap;
use std::thread;
use std::sync::{Arc, Mutex};

extern crate hyper;
extern crate uuid;
extern crate rustc_serialize;
extern crate docopt;

#[macro_use]
extern crate log;
extern crate env_logger;

use hyper::{Control, Next};
use hyper::net::{HttpListener};
use hyper::server::{Server};
use uuid::Uuid;

mod handler;

use handler::EventStream;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Client {
    id: Uuid
}

impl Client {
    fn new() -> Client {
        Client {
            id: Uuid::new_v4()
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Topic {
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

pub struct Message {
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

pub struct Manager {
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

    pub fn subscribe(&mut self, client: Client, topic: Topic, ctrl: Control) -> () {
        info!("[Manager] Subscribe client {:?} to topic {:?}", client, topic);

        // Create client's message queue
        self.messages.insert(client.clone(), Vec::new());

        // Now "subscribe" the Client and Control stream to the Topic
        self.streams.entry(topic).or_insert(Vec::new()).push((client, ctrl));
    }

    pub fn unsubscribe(&mut self, client: Client, topic: Topic) -> () {
        info!("[Manager] Unsubscribe client {:?} to topic {:?}", client, topic);

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

    pub fn publish(&mut self, topic: Topic, msg: &Vec<u8>){
        info!("[Manager] Publish to topic {:?}", topic);

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

    pub fn messages_for(&mut self, client: Client) -> Option<&mut Vec<Message>> {
        info!("[Manager] Retrieving messages for {:?}", client);

        self.messages.get_mut(&client)
    }

    //fn topic_len(&self) -> usize {
    //self.streams.len()
    //}

    //fn client_len(&self) -> usize {
    //self.messages.len()
    //}
}

docopt!(Args derive Debug, "
esper - Event Source HTTP server, powered by hyper.

Usage:
  esper [--threads=<st>]
  esper (-h | --help)
  esper --version

Options:
  -h --help       Show this screen.
  --version       Show version.
  --threads=<st>  Speed in knots [default: 2].
", flag_threads: u8);

fn main() {
    env_logger::init().unwrap();

    let args: Args = Args::docopt().decode().unwrap_or_else(|e| e.exit());
    debug!("Executing with args: {:?}", args);

    if args.flag_version {
        println!("esper v0.1.0");
        std::process::exit(0);
    }

    let listener = HttpListener::bind(&"127.0.0.1:3002".parse().unwrap()).unwrap();
    let mut handles = Vec::new();

    let mgr: Manager = Manager::new();
    let mgr_ref = Arc::new(Mutex::new(mgr));

    for _ in 0..args.flag_threads {
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
