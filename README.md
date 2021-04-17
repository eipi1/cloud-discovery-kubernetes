![Build](https://github.com/eipi1/cloud-discovery-kubernetes/actions/workflows/rust.yml/badge.svg)
[![Crates.io][crates-badge]][crates-url]
[![MIT licensed][mit-badge]][mit-url]

## Cloud Discovery Kubernetes
A Kubernetes implementation of [rust-cloud-discovery](https://github.com/eipi1/rust-cloud-discovery)

Returns list of instances in a kubernetes service. The crate uses kubernetes endpoint API 
(`/api/v1/namespaces/{namespace}/endpoints/{name}`).

### Usage
```rust
use rust_cloud_discovery::{DiscoveryClient, ServiceInstance};
use cloud_discovery_kubernetes::KubernetesDiscoverService;
#[tokio::main]
async fn main() {
    // initialize kubernetes client
    let k8s = KubernetesDiscoverService::init("demo".to_string(), "default".to_string()).await;
    if let Ok(k8s) = k8s {
        let  client = DiscoveryClient::new(k8s);
        client.get_instances().await;
    }
}
```

[crates-badge]: https://img.shields.io/crates/v/cloud-discovery-kubernetes.svg
[crates-url]: https://crates.io/crates/cluster-mode
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://github.com/tokio-rs/tokio/blob/master/LICENSE