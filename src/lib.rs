use async_trait::async_trait;
use k8s_openapi::api::core::v1::{
    EndpointSubset, Endpoints, ReadNamespacedEndpointsOptional,
};
use kube::Client;
use log::trace;
use rust_cloud_discovery::{DiscoveryService, ServiceInstance};
use std::collections::HashMap;
use std::error::Error;

pub struct KubernetesDiscoverService {
    service: String,
    namespace: String,
    client: Client,
}

impl KubernetesDiscoverService {
    pub async fn init(
        service: String,
        namespace: String,
    ) -> Result<KubernetesDiscoverService, Box<dyn Error>> {
        trace!("trying to init k8s client");
        let client = Client::try_default().await?;
        Ok(KubernetesDiscoverService {
            service,
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
        let port = subset
            .ports
            .as_ref()
            .and_then(|p| p.get(0))
            .map(|ep| ep.port as usize);

        let instances: Vec<ServiceInstance> = vec![];
        subset
            .addresses
            .as_ref()
            .map(|addresses| {
                let instances: Vec<ServiceInstance> = addresses
                    .iter()
                    .map(|address| {
                        let secure = false; //org.springframework.cloud.kubernetes.discovery.DefaultIsServicePortSecureResolver#resolve
                        let scheme = if secure { "https" } else { "http" }.to_owned();
                        let uri = uri_from_endpoint_address(&address.ip, port, &scheme);
                        ServiceInstance::new(
                            address.target_ref.as_ref().and_then(|t| t.uid.clone()),
                            None,
                            Some(address.ip.clone()),
                            port,
                            secure,
                            uri,
                            HashMap::new(),
                            Some(scheme),
                        )
                    })
                    .collect();
                instances
            })
            .or(Some(instances))
            .unwrap()
    }
}

fn uri_from_endpoint_address(
    host: &str,
    port: Option<usize>,
    scheme: &str
) -> Option<String> {
    let port = port?;
    let uri = format!("{}://{}:{}", &scheme, host, port);
    Some(uri)
}

#[async_trait]
impl DiscoveryService for KubernetesDiscoverService {
    async fn discover_instances(&self) -> Result<Vec<ServiceInstance>, Box<dyn Error>> {
        self.get_service_instance().await
        // unimplemented!()
    }
}
