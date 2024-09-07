use log::{debug, error, info};
use std::{
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    sync::Arc,
};
use simple_http::http::request;
use threadpool::ThreadPool;

fn create_socket() -> SocketAddr {
    SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::LOCALHOST), 5500)
}

fn handle_client(mut stream: TcpStream) -> io::Result<()> {
    let mut buffer = vec![0; 4096];
    stream.read(&mut buffer)?;

    let buf_str = String::from_utf8_lossy(&buffer);
    let request = request::HttpRequest::new(&buf_str)?;
    let response = request.response()?;

    debug!("{:?}", response);
    debug!("{}", String::from_utf8_lossy(&response.response_body));

    let headers = format!(
        "{} {}\r\nContent-Length: {}\r\nContent-Type: {}\r\n\r\n",
        response.version, response.status, response.content_length, response.content_type
    );

    stream.write_all(headers.as_bytes())?;
    stream.write_all(&response.response_body)?;
    stream.flush()?;

    Ok(())
}

fn server(socket: SocketAddr) -> io::Result<()> {
    let listener = TcpListener::bind(socket)?;
    let pool = ThreadPool::new(4);
    let counter = Arc::new(std::sync::Mutex::new(0));

    for stream in listener.incoming() {
        let stream = stream?;
        let counter = Arc::clone(&counter);

        pool.execute(move || {
            if let Err(e) = handle_client(stream) {
                error!("Failed to handle client: {}", e);
            } else {
                let mut counter = counter.lock().unwrap();
                *counter += 1;
                info!("Connected stream... {}", counter);
            }
        });
    }
    Ok(())
}

fn main() -> io::Result<()> {
    env_logger::init(); // Initialize the logger
    let socket = create_socket();
    server(socket)?;
    Ok(())
}