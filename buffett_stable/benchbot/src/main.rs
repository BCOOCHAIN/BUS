extern crate bincode;
#[macro_use]
extern crate clap;
extern crate influx_db_client;
extern crate rayon;
extern crate serde_json;
#[macro_use]
extern crate buffett_core;
extern crate buffett_crypto;
extern crate buffett_metrics;
extern crate buffett_timing;

use clap::{App, Arg};
use influx_db_client as influxdb;
use rayon::prelude::*;
use buffett_core::client::new_client;
use buffett_core::crdt::{Crdt, NodeInfo};
use buffett_core::token_service::DRONE_PORT;
use buffett_crypto::hash::Hash;
use buffett_core::logger;
use buffett_metrics::metrics;
use buffett_core::ncp::Ncp;
use buffett_core::service::Service;
use buffett_crypto::signature::{read_keypair, GenKeys, Keypair,KeypairUtil};
use buffett_core::system_transaction::SystemTransaction;
use buffett_core::thin_client::{sample_leader_by_gossip, ThinClient};
use buffett_timing::timing::{duration_in_milliseconds, duration_in_seconds};
use buffett_core::transaction::Transaction;
use buffett_core::wallet::request_airdrop;
use buffett_core::window::default_window;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::process::exit;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::sleep;
use std::thread::Builder;
use std::time::Duration;
use std::time::Instant;

//mvp001
use buffett_core::asciiart;
use std::io::Write; 

//mvp001
/// define the function of split_line and output "----------------------------" through the macro
fn split_line() {
    println!("------------------------------------------------------------------------------------------------------------------------");
}
//*

/// define a public structure named NodeStates with parameters tps and tx, 
/// and the parameter types both are u64 and public
pub struct NodeStats {
    pub tps: f64, 
    pub tx: u64,  
}

/// define a function named metrics_submit_token_balance whose parameter is token_balance
fn metrics_submit_token_balance(token_balance: i64) {

    /// use the submit method of the metrics crate and new a Point named "bench-tps" of influxdb,
    /// add a tag named "op" with the value of string “token_balance”,
    /// and a field named "balance" whose value is token_balance of type i64
    metrics::submit(
        influxdb::Point::new("bench-tps")
            .add_tag("op", influxdb::Value::String("token_balance".to_string()))
            .add_field("balance", influxdb::Value::Integer(token_balance as i64))
            .to_owned(),
    );
}

/// define a function named sample_txx_count with parameters exit_signal, maxes, first_tx_count, v, sample_period
fn sample_tx_count(
    exit_signal: &Arc<AtomicBool>,
    maxes: &Arc<RwLock<Vec<(SocketAddr, NodeStats)>>>,
    first_tx_count: u64,
    v: &NodeInfo,
    sample_period: u64,
) {
    /// reference to NodeInfo node information to create a new "client" of ThinClient
    let mut client = new_client(&v);
    /// get the current time 
    let mut now = Instant::now();
    /// get the initial count of transactions on the client
    let mut initial_tx_count = client.transaction_count();
    /// create the mutable variable "max_tps" and initialize it to 0.0
    let mut max_tps = 0.0;
    /// create the mutable variable named "total" 
    let mut total;

    ///  write formatted text of "tpu" to String
    let log_prefix = format!("{:21}:", v.contact_info.tpu.to_string());
    /// infinite loop
    loop {
        /// bound clinet's transactions count to the variable "tx_count"
        let tx_count = client.transaction_count();
        /// assert client's initial count of transactions >= clinet's transactions count is ture
        assert!(
            tx_count >= initial_tx_count,
            "expected tx_count({}) >= initial_tx_count({})",
            tx_count,
            initial_tx_count
        );
        /// get the amount of time elapsed since “now” was created.
        let duration = now.elapsed();
        /// get the current time 
        now = Instant::now();
        /// calculate the value of transactions count - initial count of transactions
        let sample = tx_count - initial_tx_count;
        /// copy "tx_count" into "initial_tx_count"
        initial_tx_count = tx_count;

        /// calculated the sum of the number of whole seconds contained by duration * 1_000_000_000
        /// and the fractional part of duration in nanoseconds
        let ns = duration.as_secs() * 1_000_000_000 + u64::from(duration.subsec_nanos());
        /// calculated tps vlaue by sample * 1_000_000_000 / ns 
        let tps = (sample * 1_000_000_000) as f64 / ns as f64;
        /// if tps > max_tps, then copy "tps" into "max_tps"
        if tps > max_tps {
            max_tps = tps;
        }
        /// if tx_count > first_tx_count, 
        /// then calculate the value of tx_count - first_tx_conut and bound it to toal
        if tx_count > first_tx_count {
            total = tx_count - first_tx_count;
        /// otherwise total = 0
        } else {
            total = 0;
        }
        
        
        /// starting variable named "node_role" with an underscore to avoid getting unused variable warnings
        /// and bound "Node's Roles" to "node_role"
        let _node_role="Node's Roles";
        
        if v.id == v.leader_id {
            let _node_role = "Leader   ";
        } else {
            let _node_role = "Validator";
        }
        let mut node_location = "Node Location";
        let node_ip: Vec<&str> = log_prefix.split(|c| c == '.' || c == ':').collect();
        if node_ip[0] == "192" && node_ip[1] == "168" {
            node_location = "LOCAL";
        } else if node_ip[0] == "148"
            && node_ip[1] == "153"
            && node_ip[2] == "36"
            && node_ip[3] == "220"
        {
            node_location = "US_NEW_YORK";
        } else if node_ip[0] == "148"
            && node_ip[1] == "153"
            && node_ip[2] == "50"
            && node_ip[3] == "162"
        {
            node_location = "DE_FRANKFURT";
        } else if node_ip[0] == "148"
            && node_ip[1] == "153"
            && node_ip[2] == "25"
            && node_ip[3] == "50"
        {
            node_location = "NE_ARMSTERDAM";

        } else if node_ip[0] == "164"
            && node_ip[1] == "52"
            && node_ip[2] == "39"
            && node_ip[3] == "162"
        {
            node_location = "SG_SINGAOPORE";
        } else if node_ip[0] == "118"
            && node_ip[1] == "186"
            && node_ip[2] == "39"
            && node_ip[3] == "238"
        {
            node_location = "CN_PEKING";
        }
        
        
        println!(
            "| {0:13} {1:<8} {2:3}{3:20}|{4:>15}{5:>10.2} |{6:>15}{7:>13} |{8:19}{9:9}",
            node_location,
            _node_role,
            "IP:",
            log_prefix,
            "Real-Time TPS:",
            tps,
            " Txs Proccessed:",
            sample,
            " Total Transactions:",
            total
        );

        /// sleep 0
        sleep(Duration::new(sample_period, 0));

        /// loads the value of "exit_signal" is ture (no ordering constraints, only atomic operations)
        /// print "log_prefix" through macros
        if exit_signal.load(Ordering::Relaxed) {
            println!("\n| Exit Signal detected, kill threas for this Node:{}", log_prefix);
            /// call the function of print_animation_arrows() 
            print_animation_arrows();
            /// instantiate NodeStates structure
            let stats = NodeStats {
                tps: max_tps,
                tx: total,
            };
            /// push the value of tpu and stats onto the end of "maxes"
            maxes.write().unwrap().push((v.contact_info.tpu, stats));
            /// exit the loop
            break;
        }
    }
}

/// define function named send_barrier_transaction
fn send_barrier_transaction(barrier_client: &mut ThinClient, last_id: &mut Hash, id: &Keypair) {
    /// get the current time
    let transfer_start = Instant::now();
    /// declare a mutable variable "sampel_cnt" and initialization the value of 0
    let mut sampel_cnt = 0;
    /// infinite loop
    loop {
        /// if sampel_cnt > 0 and sampel_cnt % 8 == 0
        if sampel_cnt > 0 && sampel_cnt % 8 == 0 {
        }

        /// then get ThinClient's last id and bound it to "last_id",
        /// and dereference of "last_id"
        *last_id = barrier_client.get_last_id();
        /// get the signature of transfer in ThinClient,
        /// if failed, call panic! and output the error message "Unable to send barrier transaction"
        let signature = barrier_client
            .transfer(0, &id, id.pubkey(), last_id)
            .expect("Unable to send barrier transaction");

        /// reference signature to get ThinClient's signature
        let confirmatiom = barrier_client.sample_by_signature(&signature);
        /// calculate the interval between transfer_start time and current time in milliseconds
        /// reference transfer_start's time interval to calculated the sum of
        /// the number of whole seconds contained by transfer_start's time interval * 1000
        /// and the fractional part of transfer_start's time interval in nanoseconds ／1_000_000
        let duration_ms = duration_in_milliseconds(&transfer_start.elapsed());
        /// if ThinClient'signature exists
        if confirmatiom.is_ok() {

            /// use the submit method of the metrics crate and new a Point named "bench-tps" of influxdb,
            /// add a tag named "op" with the value of string “token_balance”,
            /// and a field named "sampel_cnt" with the value of "mut sampel_cnt" whose type is Integer
            /// add a field named "duration" with the value of "duration_ms" whose type is i64
            metrics::submit(
                influxdb::Point::new("bench-tps")
                    .add_tag(
                        "op",
                        influxdb::Value::String("send_barrier_transaction".to_string()),
                    ).add_field("sampel_cnt", influxdb::Value::Integer(sampel_cnt))
                    .add_field("duration", influxdb::Value::Integer(duration_ms as i64))
                    .to_owned(),
            );

            /// get ThinClient's balance every 100 milliseconds through the pubkey, 
            /// and write the consumed time and the value of balance into the influxdb database.
            /// if the time-out is 10 seconds, then will failed to get balance, 
            /// and call panic!, and output the error message of "Failed to get balance",
            /// and write the consumed time into influxdb database     
            let balance = barrier_client
                .sample_balance_by_key_plus(
                    &id.pubkey(),
                    &Duration::from_millis(100),
                    &Duration::from_secs(10),
                ).expect("Failed to get balance");
            /// if balance !=1, then will be panic
            if balance != 1 {
                panic!("Expected an account balance of 1 (balance: {}", balance);
            }
            /// exit this loop
            break;
        }


        /// if duration_ms  > 1000 * 60 * 3
        /// then print the error message and exit the process 
        if duration_ms > 1000 * 60 * 3 {
            println!("Error: Couldn't confirm barrier transaction!");
            exit(1);
        }
        /// get a new last id of ThinClient
        let new_last_id = barrier_client.get_last_id();
        /// if new_last_id == *last_id, excute this branch
        if new_last_id == *last_id {
            /// if sampel_cnt > 0 and sampel_cnt % 8 == 0
            /// print "last_id" via dereference 
            if sampel_cnt > 0 && sampel_cnt % 8 == 0 {
                println!("last_id is not advancing, still at {:?}", *last_id);
            }
        /// otherwise, copy "new_last_id" into "last_id"
        /// and dereference of "last_id"
        } else {
            *last_id = new_last_id;
        }

        /// return the value of sampel_cnt += 1
        sampel_cnt += 1;
    }
}

/// define the function of generate_txs
fn generate_txs(
    shared_txs: &Arc<RwLock<VecDeque<Vec<Transaction>>>>,
    id: &Keypair,
    keypairs: &[Keypair],
    last_id: &Hash,
    threads: usize,
    reclaim: bool,
) {
    /// get the length of Keypair array
    let tx_count = keypairs.len();
    
    /// call the function of split_line()
    split_line();
    println!(
        "{0: <2}{1: <40}: {2: <10}",
        "|", "Transactions to be signed", tx_count
    );
    println!(
        "{0: <2}{1: <40}: {2: <10}",
        "|", "Reclaimed Tokens", reclaim
    );
    split_line();
    
    println!(
        "{0: <2}{1: <40}: {2: <60}",
        "|", "Status", "Signing Started"
    );
    /// call the function of split_line()
    split_line();
    
    /// get the current time
    let signing_start = Instant::now();
    /// traverse keypairs, generate a transaction for each keypair in keypairs，
    /// and transforms it into vector
    /// if !reclaim is true, generate a new Transaction with the parameter of id, keypair. pubkey (), 1, last_id 
    /// if !reclaim is false, generate a new Transaction with the parameter of keypair, id.pubkey(), 1, last_id 
    let transactions: Vec<_> = keypairs
        .par_iter()
        .map(|keypair| {
            if !reclaim {
                Transaction::system_new(&id, keypair.pubkey(), 1, *last_id)
            } else {
                Transaction::system_new(keypair, id.pubkey(), 1, *last_id)
            }
        }).collect();
    /// get the amount of time elapsed since “signing_start” was created
    let duration = signing_start.elapsed();
    /// calculated the sum of the number of whole seconds contained by duration * 1_000_000_000
    /// and the fractional part of duration in nanoseconds
    let ns = duration.as_secs() * 1_000_000_000 + u64::from(duration.subsec_nanos());
    /// convert tx_count and ns to f64 type and calculate the value of tx_count / ns
    let bsps = (tx_count) as f64 / ns as f64;

    /// call the function of split_line()    
    split_line();
    println!(
        "{0: <2}{1: <40}: {2: <60}",
        "|", "Status", "Signing Finished"
    );
    println!(
        "{0: <2}Transaction Generated :{1:?} ,Time Consumed:{2:.2}, Speed:{3:?} in the last {4:.2 } milliseconds",
        "|",
        tx_count,
        ns/1_000_000_000_u64,
        bsps * 1_000_000_f64 * 1000_f64,
        duration_in_milliseconds(&duration)
        
    );
    /// call the split_line() function
    split_line();

    /// new a Point named "bench-tps" of influxdb,
    /// add a tag named "op" with the value of string “generate_txs”,
    /// add a field named "duration" with the value of "duration_ms" whose type is i64 
    metrics::submit(
        influxdb::Point::new("bench-tps")
            .add_tag("op", influxdb::Value::String("generate_txs".to_string()))
            .add_field(
                "duration",
                influxdb::Value::Integer(duration_in_milliseconds(&duration) as i64),
            ).to_owned(),
    );

    /// calculate the value of the length of transactions voctor / threads
    let sz = transactions.len() / threads;
    /// Returns a voctor iterator over "sz" elements of the slice transactions voctor
    let chunks: Vec<_> = transactions.chunks(sz).collect();
    {
        /// yielding the content of an Ok with Transaction, panics if the value is an Err
        let mut shared_txs_wl = shared_txs.write().unwrap();
        /// traverse the chunks Vec, 
        /// copies chunk into a new Vec, and appends its element to the back of shared_txs_wl Vec
        for chunk in chunks {
            shared_txs_wl.push_back(chunk.to_vec());
        }
    }
}

/// define function of send_transaction
fn send_transaction(
    exit_signal: &Arc<AtomicBool>,
    shared_txs: &Arc<RwLock<VecDeque<Vec<Transaction>>>>,
    leader: &NodeInfo,
    shared_tx_thread_count: &Arc<AtomicIsize>,
    total_tx_sent_count: &Arc<AtomicUsize>,
) {
    /// reference to NodeInfo to create a new client
    let client = new_client(&leader);
    println!("| Begin to sendout transactions in parrallel");
    /// start loop
    loop {
        /// declare variables of "txs"
        let txs;
        {
            /// Get the OK result of Transaction
            let mut shared_txs_wl = shared_txs.write().unwrap();
            /// removes the first element of shared_txs_wl and returns it to "txs", 
            /// or None if the Vec is empty
            txs = shared_txs_wl.pop_front();
        }
        /// destructures "txs" into "Some(txs0)"
        /// add 1 to the current value of "shared_tx_thread_count", returning the previous value
        if let Some(txs0) = txs {
            shared_tx_thread_count.fetch_add(1, Ordering::Relaxed);

            /// get the length of txs0         
            let tx_len = txs0.len();
            /// get the current time
            let transfer_start = Instant::now();
            /// traverse txs0, reference "tx" to return transaction signature
            for tx in txs0 {
                client.transfer_signed(&tx).unwrap();
            }
            /// add -1 to the current value of "shared_tx_thread_count"
            shared_tx_thread_count.fetch_add(-1, Ordering::Relaxed);
            /// add txs0's length to the current value of "total_tx_sent_count" 
            total_tx_sent_count.fetch_add(tx_len, Ordering::Relaxed);
            println!(
                "| > 1 MU sent, to {} in {} ms, TPS: {} ",
                leader.contact_info.tpu,
                duration_in_milliseconds(&transfer_start.elapsed()),
                tx_len as f32 / duration_in_seconds(&transfer_start.elapsed()),
            );
            /// new a Point named "bench-tps" of influxdb,
            /// add a tag named "op" with the value of string "send_transactio",
            /// add a field named "duration" with the value of the amount of time elapsed since "transfer_start" was created whose type is i64 
            /// add a field named "count" with the value of "tx_len"(txs0's length) whose type is i64 
            metrics::submit(
                influxdb::Point::new("bench-tps")
                    .add_tag("op", influxdb::Value::String("send_transaction".to_string()))
                    .add_field(
                        "duration",
                        influxdb::Value::Integer(duration_in_milliseconds(&transfer_start.elapsed()) as i64),
                    ).add_field("count", influxdb::Value::Integer(tx_len as i64))
                    .to_owned(),
            );
        }
        /// determine whether to exit, if there is exit signal, then quit the loop
        if exit_signal.load(Ordering::Relaxed) {
            break;
        }
    }
}

/// define a function of "airdrop_tokens"
fn airdrop_tokens(client: &mut ThinClient, leader: &NodeInfo, id: &Keypair, tx_count: i64) {
    /// get an internet socket address, either IPv4 or IPv6
    let mut drone_addr = leader.contact_info.tpu;
    /// changes "drone_addr" port number associated with the socket address of const DRONE_PORT
    drone_addr.set_port(DRONE_PORT);
    /// get ThinClient's balance every 100 milliseconds reference to id pubkey, 
    /// and write the consumed time and the value of balance into the influxdb database.
    /// if the time-out is 1 seconds, then will failed to get balance, 
    /// and ruturn 0, and write the consumed time into influxdb database 
    let starting_balance = client.sample_balance_by_key(&id.pubkey()).unwrap_or(0);
    /// call the function of "metrics_submit_token_balance"， new a Point to the influxdb database
    metrics_submit_token_balance(starting_balance);
    /// output the value of "starting_balance" through macros
    println!("starting balance {}", starting_balance);
    /// if starting_balance < tx_count, then output，
    /// then output "| Begin to prepare data and send some Transactions:" through macros
    if starting_balance < tx_count {
        
        println!("| Begin to prepare data and send some Transactions:",);
        /// call the function of "split_line()"
        split_line();
        /// call the function of "print_animation_arrows()"
        print_animation_arrows();
        

        /// calculate the value of tx_count - starting_balance, to get airdrop amount      
        let airdrop_amount = tx_count - starting_balance;
        println!(
            "Airdropping {:?} tokens from {} for {}",
            airdrop_amount,
            drone_addr,
            id.pubkey(),
        );
        /// destructures the function of "request_airdrop" failed to get signature into "Err(e)",
        /// evaluate the block "{}"
        if let Err(e) = request_airdrop(&drone_addr, &id.pubkey(), airdrop_amount as u64) {
            panic!(
                "Error requesting airdrop: {:?} to addr: {:?} amount: {}",
                e, drone_addr, airdrop_amount
            );
        }

    
        /// copy "starting_balance" into mutable variable "current_balance"
        let mut current_balance = starting_balance;
        /// 20 cycles ( will take the values: 0, 2, ..., 19 in each iteration )
        for _ in 0..20 {
            /// sleep 500 millisenconds
            sleep(Duration::from_millis(500));
            /// get ThinClient's balance every 100 milliseconds reference to id pubkey, 
            /// and write the consumed time and the value of balance into the influxdb database.
            /// if the time-out is 1 seconds, then will failed to get balance, 
            /// will  to print "e" (the error massage) by a closure and ruturn "starting_balance",
            /// and write the consumed time into influxdb database 
            current_balance = client.sample_balance_by_key(&id.pubkey()).unwrap_or_else(|e| {
                println!("airdrop error {}", e);
                starting_balance
            });
            /// if starting_balance != current_balance，then quit the loop
            if starting_balance != current_balance {
                break;
            }
            
            println!(
                "Current balance of {} is {}...",
                id.pubkey(),
                current_balance
            );
            
        }
        /// call the function of "metrics_submit_token_balance"
        metrics_submit_token_balance(current_balance);
        if current_balance - starting_balance != airdrop_amount {
            println!(
                "Airdrop failed! {} {} {}",
                id.pubkey(),
                current_balance,
                starting_balance
            );
            /// exit the process
            exit(1);
        }
    }
}

/// define a function of "print_status_and_report"
fn print_status_and_report(
    maxes: &Arc<RwLock<Vec<(SocketAddr, NodeStats)>>>,
    _sample_period: u64,
    tx_send_elapsed: &Duration,
    _total_tx_send_count: usize,
) {
    
    let mut max_of_maxes = 0.0;
    let mut max_tx_count = 0;
    let mut nodes_with_zero_tps = 0;
    let mut total_maxes = 0.0;
    println!(" Node address        |       Max TPS | Total Transactions");
    println!("---------------------+---------------+--------------------");

    /// iterate structures of SocketAddr and NodeStats("maxes") through an Iterator
    for (sock, stats) in maxes.read().unwrap().iter() {
        /// match with NodeStats structure's field of "tx"
        let maybe_flag = match stats.tx {
            /// if tx is 0 , then return "!!!!!" to maybe_flag
            0 => "!!!!!",
            /// otherwise return ""
            _ => "",
        };

        println!(
            "{:20} | {:13.2} | {} {}",
            (*sock).to_string(),
            stats.tps,
            stats.tx,
            maybe_flag
        );

        if stats.tps == 0.0 {
            nodes_with_zero_tps += 1;
        }
        total_maxes += stats.tps;

        if stats.tps > max_of_maxes {
            max_of_maxes = stats.tps;
        }
        if stats.tx > max_tx_count {
            max_tx_count = stats.tx;
        }
    }

    if total_maxes > 0.0 {
        let num_nodes_with_tps = maxes.read().unwrap().len() - nodes_with_zero_tps;
        let average_max = total_maxes / num_nodes_with_tps as f64;
        println!("====================================================================================");
        println!("| Normal TPS:{:.2}",average_max);
        println!("====================================================================================");
        
       
    }

    println!("====================================================================================");
    println!("| Peak TPS:{:.2}",max_of_maxes);
    println!("====================================================================================");
    

    println!(
        "\tAverage TPS: {}",
        max_tx_count as f32 / duration_in_seconds(tx_send_elapsed)
    );
}


/// define a function of should_switch_directions, returns a boolean value
fn should_switch_directions(num_tokens_per_account: i64, i: i64) -> bool {
    i % (num_tokens_per_account / 4) == 0 && (i >= (3 * num_tokens_per_account) / 4)
}

/// define a function of print_animation_arrows()
fn print_animation_arrows(){
    print!("|\n|");
    /// cycle 5 times ( will take the values: 0, 2, ..., 4 in each iteration )
    for _ in 0..5 {
        print!(".");
        sleep(Duration::from_millis(300));
        /// flush this output stream, if failed, call panic! and output the error message of "some error message"
        std::io::stdout().flush().expect("some error message");
    }
    print!("\n|\n");
    
}

/// define the funtion of "leader_node_selection"
fn leader_node_selection(){
    split_line();
    println!("| {:?}","Selecting Transaction Validator Nodes from the Predefined High-Reputation Nodes List.");
    sleep(Duration::from_millis(100));
    std::io::stdout().flush().expect("some error message");
    println!("| {:?}","HRNL is populated with hundreds, even thousands of candidate nodes.");
    sleep(Duration::from_millis(100));
    std::io::stdout().flush().expect("some error message");
    println!("| {:?}","An random process is evoked to select up to 21 nodes from this list.");
    sleep(Duration::from_millis(100));
    std::io::stdout().flush().expect("some error message");
    println!("| {:?}","These 21 nodes are responsible for validating transactions on the DLT network.");
    sleep(Duration::from_millis(100));
    std::io::stdout().flush().expect("some error message");
    println!("| {:?}","They are further grouped into one leader node and 20 voting nodes.");
    sleep(Duration::from_millis(100));
    std::io::stdout().flush().expect("some error message");
    println!("| {:?}","For MVP demo, we only use 5 nodes from 5 different countries.");
    sleep(Duration::from_millis(100));
    std::io::stdout().flush().expect("some error message");
    split_line();
    sleep(Duration::from_millis(100));
    std::io::stdout().flush().expect("some error message");
    print_animation_arrows();
    split_line();
    println!("| {:?}","Transaction Validator Nodes Selection Process Complete!!");
    split_line();
}


fn main() {
    /// initialization log
    logger::setup();
    /// if there is panic in "bench-tps" program, then will record the panic information into influxdb database
    metrics::set_panic_hook("bench-tps");
    /// creates a new instance of an application named "bitconch-bench-tps" 
    /// automatically set the version of the "bitconch-bench-tps" application
    /// to the same thing as the crate at compile time througth crate_version! macro.
    /// Add arguments to the list of valid possibilities
    /// starts the parsing process, upon a failed parse an error will be displayed to the user 
    /// and the process will exit with the appropriate error code. 
    let matches = App::new("bitconch-bench-tps")
        .version(crate_version!())
        .arg(
            /// creates a new instance of Arg named "network" 
            Arg::with_name("network")
                /// sets the short version of the argument "network"
                .short("n")
                /// sets the long version of the argument "network"
                .long("network")
                /// specifies the name for value of option or positional arguments inside of help documentation
                .value_name("HOST:PORT")
                /// when running the specifies argument is "network"
                .takes_value(true)
                /// Sets the short help text of the argument， when input -h  
                /// then will output the help information
                .help("Rendezvous with the network at this gossip entry point; defaults to 127.0.0.1:8001"),
        )
        .arg(
            Arg::with_name("identity")
                .short("i")
                .long("identity")
                .value_name("PATH")
                .takes_value(true)
                .required(true)
                .help("File containing a client identity (keypair)"),
        )
        .arg(
            Arg::with_name("num-nodes")
                .short("N")
                .long("num-nodes")
                .value_name("NUM")
                .takes_value(true)
                .help("Wait for NUM nodes to converge"),
        )
        .arg(
            Arg::with_name("reject-extra-nodes")
                .long("reject-extra-nodes")
                .help("Require exactly `num-nodes` on convergence. Appropriate only for internal networks"),
        )
        .arg(
            Arg::with_name("threads")
                .short("t")
                .long("threads")
                .value_name("NUM")
                .takes_value(true)
                .help("Number of threads"),
        )
        .arg(
            Arg::with_name("duration")
                .long("duration")
                .value_name("SECS")
                .takes_value(true)
                .help("Seconds to run benchmark, then exit; default is forever"),
        )
        .arg(
            Arg::with_name("converge-only")
                .long("converge-only")
                .help("Exit immediately after converging"),
        )
        .arg(
            Arg::with_name("sustained")
                .long("sustained")
                .help("Use sustained performance mode vs. peak mode. This overlaps the tx generation with transfers."),
        )
        .arg(
            Arg::with_name("tx_count")
                .long("tx_count")
                .value_name("NUM")
                .takes_value(true)
                .help("Number of transactions to send per batch")
        )
        .get_matches();

    /// destructures "matches" into "Some(addr)", to gets the value of "network", evaluate the block "{}"
    /// if fail to parse then will output the error message, and terminates the current process with "1"
    let network = if let Some(addr) = matches.value_of("network") {
        addr.parse().unwrap_or_else(|e| {
            eprintln!("failed to parse network: {}", e);
            /// Terminates the current process with "1"
            exit(1)
        })
    /// if command line program's argument is not specified（destructure failed）, 
    /// network will not take "127.0.0.1:8001"
    } else {
        socketaddr!("127.0.0.1:8001")
    };

    /// get keypair by the parameter "identity" of the command line program,
    /// if fails，then will display the error message "can't read client identity"
    let id =
        read_keypair(matches.value_of("identity").unwrap()).expect("can't read client identity");

    /// destructures "matches" into "Some(t)", to gets the value of "threads", evaluate the block "{}"
    /// if fails to prase "threads"，then will call panic! and display the error message "can't parse threads"
    let threads = if let Some(t) = matches.value_of("threads") {
        t.to_string().parse().expect("can't parse threads")
    /// if destructure failed, then will return "4usize" to threads
    } else {
        4usize
    };

    /// destructures "matches" into "Some(n)", to gets the value of "num-nodes", evaluate the block "{}"
    /// if fails to prase "num-nodes"，then will call panic! and display the error message 
    let num_nodes = if let Some(n) = matches.value_of("num-nodes") {
        n.to_string().parse().expect("can't parse num-nodes")
    /// if destructure failed, then will return "1usize" to num_nodes
    } else {
        1usize
    };

    /// destructures "matches" into "Some(s)", gets the value of "duration", to creates a new instance of Duration
    /// if fails to prase "duration"，then will call panic! and display the error message 
    let duration = if let Some(s) = matches.value_of("duration") {
        Duration::new(s.to_string().parse().expect("can't parse duration"), 0)
    /// if destructure failed, then will return a new instance of Duration with Maximum number supported by u64
    } else {
        Duration::new(std::u64::MAX, 0)
    };

    /// destructures "matches" into "Some(s)", gets the value of "tx_count",
    /// if fails to prase "tx_count"，then will call panic! and display the error message 
    let tx_count = if let Some(s) = matches.value_of("tx_count") {
        s.to_string().parse().expect("can't parse tx_count")
    /// if destructure failed, return 500_000 to "tx_count"
    } else {
        500_000
    };

    /// returns true if an argument "sustained" was present at runtime, otherwise false.
    let sustained = matches.is_present("sustained");

    /// the `ascii_art` module implement fancy ascii arts
    asciiart::welcome();
    split_line();
    leader_node_selection();

    
    println!(
        "{0: <2}{1: <40}: {2: <60}",
        "|", "Search for Leader Node On Network", network
    );
    split_line();
    print_animation_arrows();


    /// get leader's information (NodeInfo) by the value of "network",
    /// if fails，call panic! and display the error message "unable to find leader on network"
    let leader = sample_leader_by_gossip(network, None).expect("unable to find leader on network");

    /// define exit signal, default initial value is false
    let exit_signal = Arc::new(AtomicBool::new(false));
    
    split_line();
    println!(
        "| Leader Node is found!, ID: {:?}",
        &leader.id
    );
    split_line();
    sleep(Duration::from_millis(100));
    
    /// call the function of "converge", search the effective nodes on the network
    let (nodes, leader, ncp) = converge(&leader, &exit_signal, num_nodes);

    /// if nodes.len() < num_nodes, then print the error message
    /// and exit the program
    if nodes.len() < num_nodes {
        println!(
            "Error: Insufficient nodes discovered.  Expecting {} or more",
            num_nodes
        );
        exit(1);
    }
    /// if command program's argument "reject-extra-nodes" was present at runtime, 
    /// and the length of the node > the number of nodes
    /// then print the error message and exit the program
    if matches.is_present("reject-extra-nodes") && nodes.len() > num_nodes {
        println!(
            "Error: Extra nodes discovered.  Expecting exactly {}",
            num_nodes
        );
        exit(1);
    }

    /// if "leader" is a None value, then print "no leader", and exit program
    if leader.is_none() {
        println!("no leader");
        exit(1);
    }

    /// if command line program's argument "converge-only" was present at runtime, then return it
    if matches.is_present("converge-only") {
        return;
    }

    /// if the return value "leader" of the "converge" function is encapsulated in Some, 
    /// then move "leader" out of Option < T>.
    let leader = leader.unwrap();

    //mvp001
    split_line();
    println!(
        "{0: <2}{1: <40}: {2: <60}",
        "|", "Leader Node Contact Information", leader.contact_info.rpu
    );
    println!(
        "{0: <2}{1: <40}: {2: <60}",
        "|", "Leader Node ID", leader.id
    );
    split_line();
    //*
    //println!("leader is at {} {}", leader.contact_info.rpu, leader.id);
    
    /// reference to "leader" to create two different new clients
    let mut client = new_client(&leader);
    let mut barrier_client = new_client(&leader);

    /// declare a mutable array "seed" of type u8 with 32 elements
    /// and initial values is 0
    let mut seed = [0u8; 32];
    /// copy all elements of the reference little-endian-encoded public key bytes of the id into "seed"
    seed.copy_from_slice(&id.public_key_bytes()[..32]);
    /// new a GenKeys and  instance with the parameter "seed"
    let mut rnd = GenKeys::new(seed);

    //mvp
    println!("| Begin to prepare data and send some Transactions:");
    split_line();
    print_animation_arrows();
    //println!("Creating {} keypairs...", tx_count / 2);
    println!(
        "{0: <2}{1: <40}: {2: <60}",
        "|",
        "Create Key Pairs",
        tx_count / 2
    );
    //*

    /// generate keypairs vector with the value of "tx_count / 2"
    let keypairs = rnd.gen_n_keypairs(tx_count / 2);
    /// generate the keypair vector with the parameter of "1" and pop the element to "barrier_id"
    let barrier_id = rnd.gen_n_keypairs(1).pop().unwrap();

    //mvp001
    print_animation_arrows();
    println!(
        "{0: <2}{1: <40}: {2: <60}",
        "|", "Issue Tokens", "Yes, issue some tokens to each account."
    );
    //*
    //println!("Get tokens...");
    let num_tokens_per_account = 20;

    // Sample the first keypair, see if it has tokens, if so then resume
    // to avoid token loss

    /// get ThinClient's balance every 100 milliseconds reference to the first pubkey of keypairs, 
    /// and write the consumed time and the value of balance into the influxdb database.
    /// if the time-out is 1 seconds, then will failed to get balance, a
    /// and return the default value of "0", and write the consumed time into influxdb database 
    let keypair0_balance = client.sample_balance_by_key(&keypairs[0].pubkey()).unwrap_or(0);

    /// if num_tokens_per_account > keypair0_balance, then call the function of "airdrop_tokens"
    if num_tokens_per_account > keypair0_balance {
        airdrop_tokens(
            &mut client,
            &leader,
            &id,
            (num_tokens_per_account - keypair0_balance) * tx_count,
        );
    }
    /// call the function of "airdrop_tokens"
    airdrop_tokens(&mut barrier_client, &leader, &barrier_id, 1);

    
    /// get last id of leader NodeInfo
    let mut last_id = client.get_last_id();
    

    /// get transactions count of leader NodeInfo
    let first_tx_count = client.transaction_count();
    println!("Initial transaction count {}", first_tx_count);

    
    /// creat a new vector
    let maxes = Arc::new(RwLock::new(Vec::new()));
    let sample_period = 1; 
    println!("Sampling TPS every {} second...", sample_period);
    /// create a new iterator and consume the new iterator and create a vector
    let v_threads: Vec<_> = nodes
        .into_iter()
        .map(|v| {
            /// "exit_signal" get a copy, leaving the original value in place
            let exit_signal = exit_signal.clone();
            let maxes = maxes.clone(); 
            /// generates a thread named "bitconch-client-sample"
            /// spawns a new thread by taking ownership of the Builder and call the function of "sample_tx_count"
            Builder::new()
                .name("bitconch-client-sample".to_string())
                .spawn(move || {
                    sample_tx_count(&exit_signal, &maxes, first_tx_count, &v, sample_period);
                }).unwrap()
        }).collect();

    /// constructs a new Arc<RwLock> 
    let shared_txs: Arc<RwLock<VecDeque<Vec<Transaction>>>> =
        Arc::new(RwLock::new(VecDeque::new()));

    let shared_tx_active_thread_count = Arc::new(AtomicIsize::new(0));
    let total_tx_sent_count = Arc::new(AtomicUsize::new(0));

    /// create a new iterator and consume the new iterator and create a vector
    let s_threads: Vec<_> = (0..threads)
        .map(|_| {
            /// get a copy, leaving the original value in place
            let exit_signal = exit_signal.clone();
            let shared_txs = shared_txs.clone();
            let leader = leader.clone();
            let shared_tx_active_thread_count = shared_tx_active_thread_count.clone();
            let total_tx_sent_count = total_tx_sent_count.clone();
            /// create a thread named "bitconch-client-sender", 
            /// call the function of "sample_tx_count"
            Builder::new()
                .name("bitconch-client-sender".to_string())
                .spawn(move || {
                    send_transaction(
                        &exit_signal,
                        &shared_txs,
                        &leader,
                        &shared_tx_active_thread_count,
                        &total_tx_sent_count,
                    );
                }).unwrap()
        }).collect();

    
    /// get the current time
    let start = Instant::now();
    /// define mutable variable "reclaim_tokens_back_to_source_account" with an initial value of "false"
    let mut reclaim_tokens_back_to_source_account = false;
    /// copy "keypair0_balance" into "i"
    let mut i = keypair0_balance;
    /// if the amount of time elapsed since "now" was created < command line  program's value "duration"
    while start.elapsed() < duration {
        /// then reference to id pubkey to get the balance by the public key, if failed then ruturn "-1"
        let balance = client.sample_balance_by_key(&id.pubkey()).unwrap_or(-1);
        /// call the function of "metrics_submit_token_balance"
        metrics_submit_token_balance(balance);
        /// call the function of "generate_txs"
        generate_txs(
            &shared_txs,
            &id,
            &keypairs,
            &last_id,
            threads,
            reclaim_tokens_back_to_source_account,
        );
        ///  if "!sustained" is ture
        if !sustained {
            /// while shared_tx_active_thread_count > 0, then sleep 100 milliseconds
            while shared_tx_active_thread_count.load(Ordering::Relaxed) > 0 {
                sleep(Duration::from_millis(100));
            }
        }
        /// call the function of "send_barrier_transaction"
        send_barrier_transaction(&mut barrier_client, &mut last_id, &barrier_id);

        i += 1;
        /// If the result of the function "should_switch_directions" is ture
        /// then take the opposite value of "reclaim_tokens_back_to_source_account"
        if should_switch_directions(num_tokens_per_account, i) {
            reclaim_tokens_back_to_source_account = !reclaim_tokens_back_to_source_account;
        }
    }

    /// stores a value of "ture" into the bool
    exit_signal.store(true, Ordering::Relaxed);

    split_line(); //mvp001
    println!("| Kill all the remaining threads.");
    print_animation_arrows();
    /// iteration "v_threads" vector
    for t in v_threads {
        /// if the associated thread of "v_threads" thread goes wrong when running, 
        /// then output the error information through macros
        if let Err(err) = t.join() {
            println!("  join() failed with: {:?}", err);
        }
    }

    // join the tx send threads
    //println!("Waiting for transmit threads...");
    /// iteration "s_threads" vector
    for t in s_threads {
        /// if the associated thread of "s_threads" thread goes wrong when running, 
        /// then output the error information through macros
        if let Err(err) = t.join() {
            println!("  join() failed with: {:?}", err);
        }
    }

    /// refenrenve to id pubkey to get the balance, if failed then ruturn "-1"
    let balance = client.sample_balance_by_key(&id.pubkey()).unwrap_or(-1);
    metrics_submit_token_balance(balance);

    /// call the function
    print_status_and_report(
        &maxes,
        sample_period,
        &start.elapsed(),
        total_tx_sent_count.load(Ordering::Relaxed),
    );

    // join the crdt client threads
    /// waits for the associated thread of "ncp" to finish
    ncp.join().unwrap();
}

/// define the function of converge(), whose return values type are Vec<NodeInfo>, Option<NodeInfo>, Ncp
fn converge(
    leader: &NodeInfo,
    exit_signal: &Arc<AtomicBool>,
    num_nodes: usize,
) -> (Vec<NodeInfo>, Option<NodeInfo>, Ncp) {
    //lets spy on the network
    /// creat node, gossip_socket
    /// use the "spy_node" method of the "Crdt " structure
    /// Define "node", "gossip_socket " to receive the return value of" spy_node" function
    let (node, gossip_socket) = Crdt::spy_node();
    /// create Crdt with a parameter of node, panics if the value is an Err and output "Crdt::new" 
    /// create "Crdt " instances with "node"
    let mut spy_crdt = Crdt::new(node).expect("Crdt::new");
    /// insert leader's NodeInfo into spy_crdt
    /// reference to "leader"to judge whether the node exists
    spy_crdt.insert(&leader);
    /// Set NodeInfo's id to leader's id
    /// setthe node of "spy_crdt" by leader.id
    spy_crdt.set_leader(leader.id);
    /// create Arc instances with the argument of "spy_crdt"
    /// locks this rwlock with shared read access, blocking the current thread until it can be acquired.
    let spy_ref = Arc::new(RwLock::new(spy_crdt));
    /// constructs a new Arc instances  with "default_window()"
    let window = Arc::new(RwLock::new(default_window()));
    /// create Ncp instances 
    let ncp = Ncp::new(&spy_ref, window, None, gossip_socket, exit_signal.clone());
    /// create a empty vector
    let mut v: Vec<NodeInfo> = vec![];
    // wait for the network to converge, 30 seconds should be plenty
    /// loop 30 times ( will take the values: 0, 2, ..., 29 in each iteration )
    for _ in 0..30 {
        {
            /// many reader locks can be held at once
            let spy_ref = spy_ref.read().unwrap();

            /// output the node's information through macros
            println!("{}", spy_ref.node_info_trace());

            /// if the option of "spy_ref" NodeInfois a Some value
            if spy_ref.leader_data().is_some() {
                /// get valid socket address and transforms it into vector
                v = spy_ref
                    .table
                    .values()
                    .filter(|x| Crdt::is_valid_address(&x.contact_info.rpu))
                    .cloned()
                    .collect();

                if v.len() >= num_nodes {
                    println!("CONVERGED!");
                    break;
                } else {
                    println!(
                        "{} node(s) discovered (looking for {} or more)",
                        v.len(),
                        num_nodes
                    );
                }
            }
        }
        sleep(Duration::new(1, 0));
    }
    /// clone the value of "NodeInfo"
    let leader = spy_ref.read().unwrap().leader_data().cloned();
    (v, leader, ncp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_switch_directions() {
        assert_eq!(should_switch_directions(20, 0), false);
        assert_eq!(should_switch_directions(20, 1), false);
        assert_eq!(should_switch_directions(20, 14), false);
        assert_eq!(should_switch_directions(20, 15), true);
        assert_eq!(should_switch_directions(20, 16), false);
        assert_eq!(should_switch_directions(20, 19), false);
        assert_eq!(should_switch_directions(20, 20), true);
        assert_eq!(should_switch_directions(20, 21), false);
        assert_eq!(should_switch_directions(20, 99), false);
        assert_eq!(should_switch_directions(20, 100), true);
        assert_eq!(should_switch_directions(20, 101), false);
    }
}
