use arbitrum_config::ArbitrumRethConfig;
use arbitrum_node::reth_integration::launch_reth_node;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn health_endpoint_responds() {
    let mut cfg = ArbitrumRethConfig::default();
    cfg.rpc.port = 18548; // avoid conflicts
    let handle = launch_reth_node(&cfg, None).await.expect("launch");

    let url = format!("http://127.0.0.1:{}/health", cfg.rpc.port);
    let resp = reqwest::get(url).await.expect("http get");
    assert!(resp.status().is_success());

    handle.stop().await.expect("stop");
}
