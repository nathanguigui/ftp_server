pub mod ftp_server {
    extern crate ctrlc;
    extern crate getopts;

    use std::thread;
    use std::net::{TcpListener, TcpStream, Shutdown};
    use std::io::{Read, Write};
    use std::env;
    use std::process;
    use getopts::Options;

    pub struct ParsedParams {
        port: i32,
        default_path: String,
        verbose: bool,
    }

    fn print_usage(program: &str, opts: Options) {
        let brief = format!("Usage: {} PATH [options]", program);
        print!("{}", opts.usage(&brief));
    }


    fn process_params() -> ParsedParams {
        let mut params: ParsedParams = ParsedParams { port: 3000, default_path: "/home/".to_string(), verbose: false };
        let args: Vec<String> = env::args().collect();
        let program = args[0].clone();

        let mut opts = Options::new();
        opts.optopt("p", "port", "specify port", "PORT");
        opts.optflag("h", "help", "print this help menu");
        opts.optflag("v", "verbose", "enable verbose");
        let matches = match opts.parse(&args[1..]) {
            Ok(m) => { m }
            Err(f) => { panic!(f.to_string()) }
        };
        if matches.opt_present("h") {
            print_usage(&program, opts);
            process::exit(0)
        }
        if matches.opt_present("v") {
            params.verbose = true;
        }
        let port = matches.opt_str("p");
        match port {
            Some(port) => {
                match port.parse() {
                    Ok(port) => {
                        params.port = port;
                    }
                    Err(e) => {
                        println!("Invalid arguments: {}.", e);
                        process::exit(84)
                    }
                }
            }
            None => {}
        }
        if !matches.free.is_empty() {
            params.default_path = matches.free[0].clone();
        } else {
            print_usage(&program, opts);
            process::exit(0)
        };
        return params;
    }

    fn start_server(listenner: TcpListener, params: ParsedParams) {
        if params.verbose { println!("Server listenning on port {}", params.port) };
        ctrlc::set_handler(move || {
            println!("Exiting server.");
            process::exit(0);
        }).expect("Error setting Ctrl-C handler");
        for stream in listenner.incoming() {
            match stream {
                Ok(stream) => {
                    if params.verbose { println!("New connection {}", stream.peer_addr().unwrap()) };
                    thread::spawn(move || {
                        // connection succeeded
                        handle_client(stream)
                    });
                }
                Err(e) => {
                    println!("Error: {}", e);
                }
            }
        }
        drop(listenner);
    }

    pub fn ftp_server() {
        let params = process_params();
        match TcpListener::bind(format!("0.0.0.0:{}", params.port)) {
            Ok(listenner) => {
                start_server(listenner, params);
            }
            Err(e) => {
                println!("Could not start server: {}.", e);
                process::exit(84)
            }
        };
    }

    fn handle_client(mut stream: TcpStream) {
        let mut data = [0 as u8; 50];
        let peer_ip = stream.peer_addr().unwrap();
        while match stream.read(&mut data) {
            Ok(size) => {
                if size != 0 {
                    stream.write(&data[0..size]).unwrap();
                } else {
                    println!("Client {} disconnected", peer_ip);
                    return;
                }
                true
            }
            Err(_) => {
                stream.shutdown(Shutdown::Both).unwrap();
                false
            }
        } {}
    }
}