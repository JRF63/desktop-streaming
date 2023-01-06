mod capture;
mod device;
mod nvidia;
mod input;
mod payloader;
mod server;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    env_logger::init();
    server::http_server(([192, 168, 1, 253], 9090)).await;
}
