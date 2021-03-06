extern crate bincode;
extern crate bytes;
#[macro_use]
extern crate clap;
extern crate log;
extern crate serde_json;
extern crate buffett_core;
extern crate buffett_metrics;
extern crate buffett_crypto;
extern crate tokio;
extern crate tokio_codec;

use bincode::{deserialize, serialize};
use bytes::Bytes;
use clap::{App, Arg};
use buffett_core::token_service::{Drone, DroneRequest, DRONE_PORT};
use buffett_core::logger;
use buffett_metrics::metrics::set_panic_hook;
use buffett_crypto::signature::read_keypair;
use std::error;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::process::exit;
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::net::TcpListener;
use tokio::prelude::*;
use tokio_codec::{BytesCodec, Decoder};

/// create a macro named socketaddr to genrate SocketAddr
macro_rules! socketaddr {
    /// takes the expression of type "expr"
    /// and convert tuple into a SocketAddr
    ($ip:expr, $port:expr) => {
        SocketAddr::from((Ipv4Addr::from($ip), $port))
    };
    /// take the str expression
    /// parse the str into a SocketAddr
    ($str:expr) => {{
        let a: SocketAddr = $str.parse().unwrap();
        /// retun the value of SocketAddr
        a
    }};
}

/// declare the function of main
fn main() -> Result<(), Box<error::Error>> {
    /// initialization log
    logger::setup();
    /// if there is panic in "tokenbot" program, then will record the panic information into influxdb database
    set_panic_hook("tokenbot"); 
    /// creates a new instance of an application named "tokenbot"
    /// set the version to the same version of the application as crate automatically at compile time
    /// adds an argument to the list of valid possibilities
    let matches = App::new("tokenbot")
        .version(crate_version!())
        .arg(
            /// creates a new instance of Arg named "network"
            Arg::with_name("network")
                /// sets the short version of the argument
                .short("n")
                /// sets the long version of the argument
                .long("network")
                /// specifies the name for value of option or positional arguments inside of help documentation
                .value_name("HOST:PORT")
                /// specifies that the argument takes a value at run time
                .takes_value(true)
                /// sets whether or not the argument is required by default
                .required(true)
                /// sets the short help text
                .help("Ip and port number of the leader node"),
        ).arg(
            Arg::with_name("keypair")
                .short("k")
                .long("keypair")
                .value_name("PATH")
                .takes_value(true)
                .required(true)
                .help("File from which to read the mint's keypair"),
        ).arg(
            Arg::with_name("slice")
                .long("slice")
                .value_name("SECS")
                .takes_value(true)
                .help("Time interval limit for airdropping request"),
        ).arg(
            Arg::with_name("cap")
                .long("cap")
                .value_name("NUM")
                .takes_value(true)
                .help("Request limit during each interval"),
        /// starts the parsing process, upon a failed parse an error will be displayed to the user 
        /// and the process will exit with the appropriate error code.
        ).get_matches();

    /// gets the value of "network", and parse it,
    /// if faile to parse, then will return the default error message, 
    /// print the error message, and exits the program
    let network = matches
        .value_of("network")
        .unwrap()
        .parse()
        .unwrap_or_else(|e| {
            eprintln!("failed to parse network: {}", e);
            exit(1)
        });

    /// get the keypair from the value of "keypair" in application, 
    /// and print the error message if it fails.
    let mint_keypair =
        read_keypair(matches.value_of("keypair").unwrap()).expect("failed to read client keypair");

    /// declare the "time_slice" variable
    let time_slice: Option<u64>;
    /// destructure the value of "slice" in application
    /// if "slice" wrapped Some value,then parse "slice", and return, if faile to parse then print the error message,
    /// if is None, then return None to time_slice
    if let Some(secs) = matches.value_of("slice") {
        time_slice = Some(secs.to_string().parse().expect("failed to parse slice"));
    } else {
        time_slice = None;
    }
    /// declare the "request_cap" variable
    let request_cap: Option<u64>;
    /// destructure the value of "cap" in application
    /// if "cap" wrapped Some value,then parse "cap", and return, if faile to parse then print the error message,
    /// if is None, then return None to request_cap
    if let Some(c) = matches.value_of("cap") {
        request_cap = Some(c.to_string().parse().expect("failed to parse cap"));
    } else {
        request_cap = None;
    }

    /// get the airdrop address
    let drone_addr = socketaddr!(0, DRONE_PORT);

    /// generate a instance of Drone
    /// using an Arc<T> to wrap the Mutex<T> able to share ownership across multiple threads
    let drone = Arc::new(Mutex::new(Drone::new(
        mint_keypair,
        drone_addr,
        network,
        time_slice,
        request_cap,
    )));

    /// makes a clone of the "drone" Arc pointer
    let drone1 = drone.clone();
    /// create a new thread to loop
    thread::spawn(move || loop {
        /// use "lock" method to get locks to access "time_slice" data in "drone1" mutex
        let time = drone1.lock().unwrap().time_slice;
        /// puts the current thread to sleep
        thread::sleep(time);
        /// obtain locks to access the results of "clear_request_count" function in the "drone1" mutex
        drone1.lock().unwrap().clear_request_count();
    });

    /// create a new TCP listener associated with this event loop
    let socket = TcpListener::bind(&drone_addr).unwrap();
    println!("Tokenbot started. Listening on: {}", drone_addr);
    /// consumes "socket" listener, returning a stream of the sockets listener accepts
    /// and calls a closure on each element of "socket" iterator
    /// if faile to consumes "socket" listener, then print the error message
    let done = socket
        .incoming()
        .map_err(|e| println!("failed to accept socket; error = {:?}", e))
        .for_each(move |socket| {
            /// makes a clone of the "drone" Arc pointer
            let drone2 = drone.clone();
            /// creates a new BytesCodec for shipping around raw bytes
            /// and provides a Stream and Sink interface for reading and writing to the Io object
            // let client_ip = socket.peer_addr().expect("drone peer_addr").ip();
            let framed = BytesCodec::new().framed(socket);
            /// break the Stream and Sink interface into separate objects, allowing them to interact more easily
            let (writer, reader) = framed.split();

            /// returns None if the option of "reader" is None,
            /// otherwise calls closure with the wrapped value and returns the result
            let processor = reader.and_then(move |bytes| {
                /// deserializes a slice of bytes into an instance of DroneRequest using the default configuration
                /// if faile to deserializes, then call the closure
                let req: DroneRequest = deserialize(&bytes).or_else(|err| {
                    /// creates a new I/O error from formatted string error as well as an arbitrary error payload
                    Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("deserialize packet in drone: {:?}", err),
                    ))
                })?;

                println!("Airdrop requested...");
                /// obtain locks to access the results of "send_airdrop(req)" function in the "drone2" mutex
                // let res = drone2.lock().unwrap().check_rate_limit(client_ip);
                let res1 = drone2.lock().unwrap().send_airdrop(req);
                match res1 {
                    Ok(_) => println!("Airdrop sent!"),
                    Err(_) => println!("Request limit reached for this time slice"),
                }
                let response = res1?;
                println!("Airdrop tx signature: {:?}", response);
                /// serializes "response" into a Vec of bytes using the default configuration.
                /// if faile to serialize, then call the closure
                let response_vec = serialize(&response).or_else(|err| {
                    /// creates a new I/O error from formatted string error as well as an arbitrary error payload
                    Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("serialize signature in drone: {:?}", err),
                    ))
                })?;
                /// creates a new Bytes from clone "response_vec"
                let response_bytes = Bytes::from(response_vec.clone());
                /// return a OK value of "response_bytes"
                Ok(response_bytes)
            });
            /// process the stream of "processor" into the sink, including flushing
            /// if processor is None or occurs error, then call the closure
            let server = writer
                .send_all(processor.or_else(|err| {
                    /// creates a new I/O error from formatted string error as well as an arbitrary error payload
                    Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("Tokenbot response: {:?}", err),
                    ))
                })).then(|_| Ok(()));
            /// spawns a future or stream, returning it and the new task responsible for running it to completion
            tokio::spawn(server)
        });
    /// start the Tokio runtime using the supplied future to bootstrap execution
    tokio::run(done);
    /// return the OK value
    Ok(())
}
