pub mod ftp_server {
    extern crate ctrlc;
    extern crate getopts;

    use std::{thread, fs};
    use std::net::{TcpListener, TcpStream, Shutdown};
    use std::io::{Read, Write};
    use std::env;
    use std::process;
    use getopts::Options;
    use std::str;
    use std::path::Path;

    struct ParsedParams {
        port: i32,
        default_path: String,
        verbose: bool,
    }

    struct ConnectionState<'a> {
        username: String,
        connected: bool,
        current_path: &'a Path,
    }

    struct ParsedInput {
        argv: Vec<String>,
        input: String,
    }

    fn print_usage(program: &str, opts: Options) {
        let brief = format!("Usage: {} PATH [options]", program);
        print!("{}", opts.usage(&brief));
    }

    fn process_params() -> ParsedParams {
        let mut params: ParsedParams = ParsedParams { port: 3000, default_path: ".".to_string(), verbose: false };
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
                    let base_path = params.default_path.clone();
                    thread::spawn(move || {
                        // connection succeeded
                        handle_client(stream, base_path)
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

    fn parse_user_input(client_input: &str) -> ParsedInput {
        let new_input = client_input.replace("\r\n", "").replace("\n", "");
        let argv = new_input.split_whitespace().map(|s| s.to_string()).collect();
        let result: ParsedInput = ParsedInput {
            argv,
            input: new_input,
        };
        return result;
    }

    fn handle_user_command(mut stream: &TcpStream, client_state: &mut ConnectionState, parsed_input: ParsedInput) {
        let tmp = format!("{} ", parsed_input.argv[0]).to_string();
        let split_name = parsed_input.input.split(&tmp);
        let name: Vec<&str> = split_name.collect();
        if parsed_input.argv.len() < 2 {
            match stream.write("530 Permission denied.\r\n".as_bytes()) {
                Ok(_) => {}
                _ => {}
            }
        } else {
            match stream.write("331 Please specify the password.\r\n".as_bytes()) {
                Ok(_) => {}
                _ => {}
            }
            client_state.username = name[1].to_string();
        }
    }

    fn handle_pass_command(mut stream: &TcpStream, client_state: &mut ConnectionState, _parsed_input: ParsedInput) {
        if client_state.username.to_uppercase() == "ANONYMOUS".to_string() {
            client_state.connected = true;
            match stream.write("230 Login successful.\r\n".as_bytes()) {
                Ok(_) => {}
                _ => {}
            }
        } else {
            match stream.write("530 Login incorrect.\r\n".as_bytes()) {
                Ok(_) => {}
                _ => {}
            }
        }
    }

    fn handle_unauth_commands(mut stream: &TcpStream, parsed_input: ParsedInput, client_state: &mut ConnectionState) {
        if parsed_input.argv[0].to_uppercase() == "USER".to_string() {
            handle_user_command(stream, client_state, parsed_input);
        } else if client_state.username.len() != 0 && parsed_input.argv[0].to_uppercase() == "PASS".to_string() {
            handle_pass_command(stream, client_state, parsed_input);
        } else {
            match stream.write("530 Please login with USER and PASS.\r\n".as_bytes()) {
                Ok(_) => {}
                _ => {}
            };
        }
    }

    fn handle_auth_commands(mut stream: &TcpStream, parsed_input: ParsedInput, client_state: &mut ConnectionState) {
        if parsed_input.argv[0].to_uppercase() == "HELP".to_string() {
            stream.write("214-The following commands are recognized.\r\n".as_bytes()).unwrap();
            stream.write("214 Help OK.\r\n".as_bytes()).unwrap();
        }
        if parsed_input.argv[0].to_uppercase() == "PWD".to_string() {
            stream.write(format!("257 {:?}\r\n", fs::canonicalize(client_state.current_path).unwrap()).as_bytes()).unwrap();
        }
    }

    fn handle_quit_command(mut stream: &TcpStream, parsed_input: &ParsedInput, client_state: &mut ConnectionState) {
        if *parsed_input.argv[0].to_uppercase() == "QUIT".to_string() {
            match stream.write("221 Goodbye.\r\n".as_bytes()) {
                Ok(_) => {}
                _ => {}
            };
            stream.shutdown(Shutdown::Both).unwrap();
        }
    }

    fn handle_commands(stream: &TcpStream, client_input: &str, client_state: &mut ConnectionState) {
        let parsed_input = parse_user_input(client_input);
        handle_quit_command(&stream, &parsed_input, client_state);
        if client_state.connected {
            handle_auth_commands(&stream, parsed_input, client_state);
        } else {
            handle_unauth_commands(&stream, parsed_input, client_state);
        }
    }

    fn handle_client(mut stream: TcpStream, base_path: String) {
        let mut data = [0 as u8; 65535];
        let mut state: ConnectionState = ConnectionState {
            username: "".to_string(),
            connected: false,
            current_path: Path::new(&base_path),
        };
        let peer_ip = stream.peer_addr().unwrap();
        match stream.write("220 Rust FTP.\r\n".as_bytes()) {
            Ok(_) => {}
            _ => {}
        }
        while match stream.read(&mut data) {
            Ok(size) => {
                if size != 0 {
                    handle_commands(&stream, str::from_utf8(&data[0..size]).unwrap(), &mut state);
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