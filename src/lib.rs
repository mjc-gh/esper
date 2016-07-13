use std::collections::HashMap;

extern crate rustc_serialize;
extern crate hyper;
extern crate uuid;

#[macro_use]
extern crate log;

use hyper::{Control, Next};
use rustc_serialize::json::{self, EncodeResult};

use uuid::Uuid;

pub mod handler;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Client {
    id: Uuid
}

impl Client {
    pub fn new() -> Client {
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
    pub fn empty() -> Topic {
        Topic {
            id: None
        }
    }

    pub fn validate(skip: usize, full_path: &String) -> Option<Topic> {
        let id: String = full_path.chars().skip(skip).collect();

        match (id.chars().all(|c| c.is_alphanumeric()), id.len()) {
            (true, 8...64) => Some(Topic {
                id: Some(id.to_lowercase().into_boxed_str())
            }),

            _ => None
        }
    }
}

#[derive(Clone)]
pub struct Message {
    body: Vec<u8>
}

impl Message {
    pub fn new(buf: &Vec<u8>) -> Message {
        // Append Event Source delimiter ("\n\n")
        let mut delimiter = vec![10 as u8, 10 as u8];
        let mut delimited_body = buf.clone();

        delimited_body.append(&mut delimiter);

        Message {
            body: delimited_body
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        self.body.as_slice()
    }
}

#[derive(RustcEncodable)]
pub struct Stats {
    pub clients: usize,
    pub topics: usize
}

pub struct Manager {
    messages: HashMap<Client, Vec<Message>>,
    streams: HashMap<Topic, Vec<(Client, Control)>>
}

impl Manager {
    pub fn new() -> Manager {
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

    pub fn publish(&mut self, topic: Topic, msg: &Vec<u8>) -> () {
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

    pub fn messages_for(&mut self, client: Client) -> Vec<Message> {
        info!("[Manager] Retrieving messages for {:?}", client);

        match self.messages.get_mut(&client) {
            Some (mut msgs) => {
                let ret_msgs = msgs.split_off(0);

                msgs.clear();

                ret_msgs
            }

            None => Vec::new()
        }
    }

    pub fn stats(&self) -> Stats {
        Stats {
            clients: self.messages.len(),
            topics: self.streams.len()
        }
    }

    pub fn stats_json(&self) -> EncodeResult<String> {
        json::encode(&self.stats())
    }
}
