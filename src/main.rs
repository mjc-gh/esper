#![feature(plugin)]
#![plugin(docopt_macros)]

extern crate rustc_serialize;
extern crate docopt;
extern crate hyper;

#[macro_use]
extern crate log;
extern crate env_logger;

extern crate esper;

use std::thread;
use std::sync::{Arc, Mutex};
use std::error::Error;

use hyper::net::{HttpListener};
use hyper::server::{Server};

use esper::{Access, Manager};
use esper::handler::EventStream;

docopt!(Args derive Debug, "
esper - Event Source HTTP server, powered by hyper.

Usage:
  esper [--bind=<bind>] [--port=<port>] [--threads=<st>]
  esper (-h | --help)
  esper (-v | --version)

Options:
  -h --help          Show this screen.
  -v --version       Show version.
  -b --bind=<bind>   Bind to specific IP [default: 127.0.0.1]
  -p --port=<port>   Run on a specific port number [default: 3000]
  -t --threads=<st>  Number of server threads [default: 2].
", flag_threads: u8);

fn abort(message: &'static str) -> () {
    println!("{}", message);
    std::process::exit(0);
}

fn main() {
    println!("Welcome to esper -- the Event Source HTTP server, powered by hyper!\n");
    env_logger::init().unwrap_or_else(|_| abort("Failed to initialize logger!"));

    let args: Args = Args::docopt().decode().unwrap_or_else(|e| e.exit());
    debug!("Executing with args: {:?}", args);

    if args.flag_version {
        abort("esper v0.1.0");
    }

    match format!("{}:{}", args.flag_bind, args.flag_port).parse() {
        Ok(addr) => {
            match HttpListener::bind(&addr) {
                Ok(http_listener) => {
                    let mut handles = Vec::new();

                    let mgr_ref = Arc::new(Mutex::new(Manager::new()));
                    let acc_ref = Arc::new(Access::from_env());

                    for _ in 0..args.flag_threads {
                        match http_listener.try_clone() {
                            Ok(thread_listener) => {
                                let acc_inner = acc_ref.clone();
                                let mgr_inner = mgr_ref.clone();

                                handles.push(thread::spawn(move || {
                                    let server = Server::new(thread_listener).handle(|ctrl| {
                                        EventStream::new(ctrl, acc_inner.clone(), mgr_inner.clone())
                                    });

                                    match server {
                                        Ok(_) => debug!("Started server thread"),
                                        Err(err) => warn!("Failed to start server thread; {:?}", err.description())
                                    }
                                }));
                            }

                            Err(err) => {
                                warn!("Failed to clone listener for thread; {:?}", err.description());
                            }
                        }
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }
                }

                Err(err) => {
                    println!("Http listener failed; {:?}", err.description());
                    std::process::exit(0);
                }
            }
        }

        Err(err) => {
            println!("Address parse error; {:?}", err.description());
            std::process::exit(0);
        }
    }
}
