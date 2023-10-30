use ed25519_dalek::ed25519::signature::digest::{consts::U32, generic_array::GenericArray};
use multi_party_eddsa::protocols::ExpandedKeyPair;
use near_workspaces::AccountId;
use testcontainers::{
    core::{ExecCommand, WaitFor},
    Container, GenericImage, RunnableImage,
};
use tracing;

pub struct Node<'a> {
    pub container: Container<'a, GenericImage>,
    pub address: String,
    pub local_address: String,
}

pub struct NodeApi {
    pub address: String,
    pub node_id: usize,
    pub sk_share: ExpandedKeyPair,
    pub cipher_key: GenericArray<u8, U32>,
    pub gcp_project_id: String,
    pub gcp_datastore_local_url: String,
}

impl<'a> Node<'a> {
    // Container port used for the docker network, does not have to be unique
    const CONTAINER_PORT: u16 = 3000;

    pub async fn run(
        ctx: &super::Context<'a>,
        node_id: u32,
        account: &AccountId,
        account_sk: &near_workspaces::types::SecretKey,
    ) -> anyhow::Result<Node<'a>> {
        tracing::info!(node_id, "running node container");
        let args = mpc_recovery_node::cli::Cli::Start {
            node_id: node_id.into(),
            near_rpc: ctx.sandbox.local_address.clone(),
            mpc_contract_id: ctx.mpc_contract.id().clone(),
            account: account.clone(),
            account_sk: account_sk.to_string().parse()?,
            web_port: Self::CONTAINER_PORT,
        }
        .into_str_args();
        let image: GenericImage = GenericImage::new("near/mpc-recovery-node", "latest")
            .with_wait_for(WaitFor::Nothing)
            .with_exposed_port(Self::CONTAINER_PORT)
            .with_env_var("RUST_LOG", "mpc_recovery_node=DEBUG")
            .with_env_var("RUST_BACKTRACE", "1");
        let image: RunnableImage<GenericImage> = (image, args).into();
        let image = image.with_network(&ctx.docker_network);
        let container = ctx.docker_client.cli.run(image);
        let ip_address = ctx
            .docker_client
            .get_network_ip_address(&container, &ctx.docker_network)
            .await?;
        let host_port = container.get_host_port_ipv4(Self::CONTAINER_PORT);

        container.exec(ExecCommand {
            cmd: format!("bash -c 'while [[ \"$(curl -s -o /dev/null -w ''%{{http_code}}'' localhost:{})\" != \"200\" ]]; do sleep 1; done'", Self::CONTAINER_PORT),
            ready_conditions: vec![WaitFor::message_on_stdout("node is ready to accept connections")]
        });

        let full_address = format!("http://{ip_address}:{}", Self::CONTAINER_PORT);
        tracing::info!(node_id, full_address, "node container is running");
        Ok(Node {
            container,
            address: full_address,
            local_address: format!("http://localhost:{host_port}"),
        })
    }
}