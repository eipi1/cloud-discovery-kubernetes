#![warn(missing_docs)]

//!A Kubernetes implementation of [rust-cloud-discovery](https://github.com/eipi1/rust-cloud-discovery)
//!
//! Returns list of instances in a kubernetes service. The crate uses kubernetes
//! [endpoint API](https://kubernetes.io/docs/reference/kubernetes-api/service-resources/endpoints-v1/#get-read-the-specified-endpoints)
//! (`/api/v1/namespaces/{namespace}/endpoints/{name}`).
//!
//! ### Usage
//! ```no_run
//! # use cloud_discovery_kubernetes::KubernetesDiscoverService;
//! # use rust_cloud_discovery::{DiscoveryClient, ServiceInstance};
//! # #[tokio::main]
//! # async fn main() {
//!     // initialize kubernetes client
//!     let k8s = KubernetesDiscoverService::init("demo".to_string(), "default".to_string()).await;
//!     if let Ok(k8s) = k8s {
//!         let client = DiscoveryClient::new(k8s);
//!         client.get_instances().await;
//!     }
//! # }
//! ```

use async_trait::async_trait;
use k8s_openapi::api::core::v1::{EndpointSubset, Endpoints, ReadNamespacedEndpointsOptional};
use kube::Client;
use log::trace;
use rust_cloud_discovery::{DiscoveryService, Port, ServiceInstance};
use std::collections::HashMap;
use std::error::Error;
use std::string::ToString;

const WELL_KNOWN_SECURE_PORTS: [u32; 2] = [443u32, 8443u32];
const WELL_KNOWN_HTTP_PORTS: [u32; 2] = [80u32, 8080u32];
const SECURE_APP_PROTOCOL: &str = "https";
const HTTP_APP_PROTOCOL: &str = "http";

/// A Kubernetes implementation of [rust-cloud-discovery](https://github.com/eipi1/rust-cloud-discovery)
pub struct KubernetesDiscoverService {
    service: String,
    namespace: String,
    client: Client,
}

impl KubernetesDiscoverService {
    /// Initialize the discovery service
    /// # Arguments
    /// * service - name of the endpoints
    /// * namespace - Kubernetes namespace
    pub async fn init(
        name: String,
        namespace: String,
    ) -> Result<KubernetesDiscoverService, Box<dyn Error>> {
        trace!("trying to init k8s client");
        let client = Client::try_default().await?;
        Ok(KubernetesDiscoverService {
            service: name,
            namespace,
            client,
        })
    }
    async fn get_service_instance(&self) -> Result<Vec<ServiceInstance>, Box<dyn Error>> {
        let request = k8s_openapi::api::core::v1::Endpoints::read_namespaced_endpoints(
            &self.service,
            &self.namespace,
            ReadNamespacedEndpointsOptional::default(),
        );
        let (request, _) = request.unwrap();
        let endpoints = self.client.request::<Endpoints>(request).await?;
        trace!("k8s endpoint response: {:?}", &endpoints);
        let instances = Self::endpoints_to_service_instance(&endpoints);
        trace!("k8s service instances: {:?}", &instances);
        Ok(instances)
    }

    fn endpoints_to_service_instance(endpoints: &Endpoints) -> Vec<ServiceInstance> {
        let subsets = &vec![];
        let subsets = match endpoints.subsets.as_ref() {
            Some(s) => s,
            None => subsets,
        };

        let mut instances = vec![];
        for subset in subsets {
            instances.append(&mut Self::subset_to_service_instances(subset));
        }
        instances
    }

    fn subset_to_service_instances(subset: &EndpointSubset) -> Vec<ServiceInstance> {
        let ports = subset
            .ports
            .as_ref()
            .into_iter()
            .flatten()
            .map(|port| {
                Port::new(
                    port.name.clone(),
                    port.port as u32,
                    port.protocol.clone().unwrap_or_else(|| "TCP".to_string()),
                    port.app_protocol.clone(),
                )
            })
            .collect::<Vec<_>>();

        // set default port in priority
        // https > http > others
        // if there's multiple ports of same kind, first one in the list has higher priority
        let mut default_port = None;
        let mut has_secure = false;
        let mut has_http = false;
        for port in &ports {
            if default_port.is_none() {
                default_port = Some(port.get_port());
            } else if is_secure(port) {
                default_port = Some(port.get_port());
                has_secure = true;
                break;
            } else if !has_http && is_http(port) {
                default_port = Some(port.get_port());
                has_http = true;
            }
        }

        let instances: Vec<ServiceInstance> = vec![];
        subset
            .addresses
            .as_ref()
            .map(|addresses| {
                let instances: Vec<ServiceInstance> = addresses
                    .iter()
                    .map(|address| {
                        let scheme = if has_secure { "https" } else { "http" }.to_owned();
                        let uri = uri_from_endpoint_address(&address.ip, default_port, &scheme);
                        ServiceInstance::new(
                            address.target_ref.as_ref().and_then(|t| t.uid.clone()),
                            None,
                            Some(address.ip.clone()),
                            Some(ports.clone()),
                            has_secure,
                            uri,
                            HashMap::new(),
                            Some(scheme),
                        )
                    })
                    .collect();
                instances
            })
            .unwrap_or_else(|| instances)
    }
}

fn is_secure(port: &Port) -> bool {
    let port_n = port.get_port();
    let app_protocol = port
        .get_app_protocol()
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or_else(|| "");
    let name = port
        .get_name()
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or_else(|| "");

    WELL_KNOWN_SECURE_PORTS.contains(&port_n) //well known secure ports
        || app_protocol == SECURE_APP_PROTOCOL //https://kubernetes.io/docs/concepts/services-networking/service/#application-protocol
        || name.starts_with(SECURE_APP_PROTOCOL) // or port has a name that starts with https
}

fn is_http(port: &Port) -> bool {
    let port_n = port.get_port();
    let app_protocol = port
        .get_app_protocol()
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or_else(|| "");
    let name = port
        .get_name()
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or_else(|| "");

    WELL_KNOWN_HTTP_PORTS.contains(&port_n) //well known secure ports
        || app_protocol == HTTP_APP_PROTOCOL //https://kubernetes.io/docs/concepts/services-networking/service/#application-protocol
        || name.starts_with(HTTP_APP_PROTOCOL) // or port has a name that starts with https
}

fn uri_from_endpoint_address(host: &str, port: Option<u32>, scheme: &str) -> Option<String> {
    let port = port?;
    let uri = format!("{}://{}:{}", &scheme, host, port);
    Some(uri)
}

#[async_trait]
impl DiscoveryService for KubernetesDiscoverService {
    /// Return list of Kubernetes endpoints as `ServiceInstance`s
    async fn discover_instances(&self) -> Result<Vec<ServiceInstance>, Box<dyn Error>> {
        self.get_service_instance().await
    }
}
