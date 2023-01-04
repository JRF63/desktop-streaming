mod capture;
mod device;
mod encoder;
mod input;
// mod server;

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    env_logger::init();
    // server::http_server(([127, 0, 0, 1], 9090)).await;
}