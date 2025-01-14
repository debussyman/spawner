use crate::{pod_id::PodId, SpawnerSettings};
use k8s_openapi::api::core::v1::{
    Container, ContainerPort, EnvVar, Pod, PodSpec, Service, ServicePort, ServiceSpec,
};
use kube::{
    api::{Api, DeleteParams, ObjectMeta, PostParams},
    Client,
};

const TCP: &str = "TCP";
const LABEL_RUN: &str = "run";
const LABEL_ACCOUNT: &str = "account";
const ENV_SPAWNER_POD_NAME: &str = "SPAWNER_POD_NAME";
const ENV_SPAWNER_POD_URL: &str = "SPAWNER_POD_URL";
const APPLICATION: &str = "app";
const MONITOR: &str = "monitor";

pub async fn delete_pod(pod_id: &PodId, namespace: &str) -> Result<(), kube::error::Error> {
    let client = Client::try_default().await?;
    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let services: Api<Service> = Api::namespaced(client, namespace);

    pods.delete(&pod_id.prefixed_name(), &DeleteParams::default())
        .await?;
    services
        .delete(&pod_id.prefixed_name(), &DeleteParams::default())
        .await?;

    Ok(())
}

pub async fn create_pod(
    account: Option<&str>,
    settings: &SpawnerSettings,
) -> Result<PodId, kube::error::Error> {
    let SpawnerSettings {
        application_image,
        sidecar_image,
        application_port,
        sidecar_port,
        namespace,
        ..
    } = settings;

    let pod_id = PodId::new();
    let pod_url = settings.url_for(&pod_id);
    let client = Client::try_default().await?;

    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let services: Api<Service> = Api::namespaced(client, namespace);

    let mut containers = vec![Container {
        name: APPLICATION.to_string(),
        image: Some(application_image.to_string()),
        image_pull_policy: Some("IfNotPresent".to_string()),
        // NB containerPort is primarily informational, Kubernetes does not use it, but
        // other software may read it.
        // https://kubernetes.io/docs/reference/generated/kubernetes-api/v1.19/#container-v1-core
        ports: Some(vec![ContainerPort {
            container_port: *application_port as _,
            ..ContainerPort::default()
        }]),
        env: Some(vec![
            EnvVar {
                name: ENV_SPAWNER_POD_NAME.to_string(),
                value: Some(pod_id.name()),
                ..EnvVar::default()
            },
            EnvVar {
                name: ENV_SPAWNER_POD_URL.to_string(),
                value: Some(pod_url.to_string()),
                ..EnvVar::default()
            },
        ]),
        ..Container::default()
    }];

    if let Some(sidecar_image) = sidecar_image {
        containers.push(Container {
            name: MONITOR.to_string(),
            image: Some(sidecar_image.to_string()),
            ports: Some(vec![ContainerPort {
                container_port: *sidecar_port as _,
                ..ContainerPort::default()
            }]),
            ..Container::default()
        });
    }

    pods.create(
        &PostParams::default(),
        &Pod {
            metadata: ObjectMeta {
                name: Some(pod_id.prefixed_name()),
                labels: Some(
                    vec![
                        (LABEL_RUN.to_string(), pod_id.prefixed_name()),
                        (
                            LABEL_ACCOUNT.to_string(),
                            account.to_owned().unwrap_or_default().to_string(),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                ..ObjectMeta::default()
            },

            spec: Some(PodSpec {
                containers,
                ..PodSpec::default()
            }),

            ..Pod::default()
        },
    )
    .await?;

    let mut service_ports = vec![ServicePort {
        name: Some(APPLICATION.to_string()),
        protocol: Some(TCP.to_string()),
        port: *application_port as _,
        ..ServicePort::default()
    }];

    if sidecar_image.is_some() {
        service_ports.push(ServicePort {
            name: Some(MONITOR.to_string()),
            protocol: Some(TCP.to_string()),
            port: *sidecar_port as _,
            ..ServicePort::default()
        });
    }

    services
        .create(
            &PostParams::default(),
            &Service {
                metadata: ObjectMeta {
                    name: Some(pod_id.prefixed_name()),
                    ..ObjectMeta::default()
                },

                spec: Some(ServiceSpec {
                    selector: Some(
                        vec![(LABEL_RUN.to_string(), pod_id.prefixed_name())]
                            .into_iter()
                            .collect(),
                    ),

                    ports: Some(service_ports),

                    ..ServiceSpec::default()
                }),

                ..Default::default()
            },
        )
        .await?;

    Ok(pod_id)
}
