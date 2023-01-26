mod capture;
mod device;
mod input;
mod nvidia;
mod server;
mod signaler;

use std::net::SocketAddr;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    env_logger::init();
    let port: u16 = 9090;
    let socket_addr: SocketAddr = ([0, 0, 0, 0], port).into();
    println!("Serving from http://{socket_addr}");
    server::http_server(socket_addr).await;
}
