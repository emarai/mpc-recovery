pub mod containers;
pub mod local;

use crate::env::containers::DockerClient;
use crate::{initialize_lake_indexer, LakeIndexerCtx};
use mpc_contract::primitives::CandidateInfo;
use near_workspaces::network::Sandbox;
use near_workspaces::{AccountId, Contract, Worker};
use serde_json::json;
use std::collections::HashMap;

const NETWORK: &str = "mpc_it_network";

pub enum Nodes<'a> {
    Local {
        ctx: Context<'a>,
        nodes: Vec<local::Node>,
    },
    Docker {
        ctx: Context<'a>,
        nodes: Vec<containers::Node<'a>>,
    },
}

impl Nodes<'_> {
    pub fn len(&self) -> usize {
        match self {
            Nodes::Local { nodes, .. } => nodes.len(),
            Nodes::Docker { nodes, .. } => nodes.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn ctx(&self) -> &Context {
        match self {
            Nodes::Local { ctx, .. } => ctx,
            Nodes::Docker { ctx, .. } => ctx,
        }
    }

    pub fn url(&self, id: usize) -> &str {
        match self {
            Nodes::Local { nodes, .. } => &nodes[id].address,
            Nodes::Docker { nodes, .. } => &nodes[id].address,
        }
    }

    pub async fn add_node(
        &mut self,
        account: &AccountId,
        account_sk: &near_workspaces::types::SecretKey,
    ) -> anyhow::Result<()> {
        tracing::info!(%account, "adding one more node");
        match self {
            Nodes::Local { ctx, nodes } => {
                nodes.push(local::Node::run(ctx, account, account_sk).await?)
            }
            Nodes::Docker { ctx, nodes } => {
                nodes.push(containers::Node::run(ctx, account, account_sk).await?)
            }
        }

        Ok(())
    }
}

pub struct Context<'a> {
    pub docker_client: &'a DockerClient,
    pub docker_network: String,
    pub release: bool,

    pub localstack: crate::env::containers::LocalStack<'a>,
    pub lake_indexer: crate::env::containers::LakeIndexer<'a>,
    pub worker: Worker<Sandbox>,
    pub mpc_contract: Contract,
}

pub async fn setup(docker_client: &DockerClient) -> anyhow::Result<Context<'_>> {
    if !crate::mpc::build_multichain_contract().await?.success() {
        anyhow::bail!("failed to prebuild multichain contract");
    }

    let release = true;
    if !crate::mpc::build_multichain(release).await?.success() {
        anyhow::bail!("failed to prebuild multichain node service");
    }

    let docker_network = NETWORK;
    docker_client.create_network(docker_network).await?;

    let LakeIndexerCtx {
        localstack,
        lake_indexer,
        worker,
    } = initialize_lake_indexer(docker_client, docker_network).await?;

    let mpc_contract = worker
        .dev_deploy(&std::fs::read(
            "../target/wasm32-unknown-unknown/release/mpc_contract.wasm",
        )?)
        .await?;
    tracing::info!(contract_id = %mpc_contract.id(), "deployed mpc contract");

    Ok(Context {
        docker_client,
        docker_network: docker_network.to_string(),
        release,
        localstack,
        lake_indexer,
        worker,
        mpc_contract,
    })
}

pub async fn docker(nodes: usize, docker_client: &DockerClient) -> anyhow::Result<Nodes> {
    let ctx = setup(docker_client).await?;

    let accounts = futures::future::join_all((0..nodes).map(|_| ctx.worker.dev_create_account()))
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let mut node_futures = Vec::new();
    for account in &accounts {
        let node = containers::Node::run(&ctx, account.id(), account.secret_key());
        node_futures.push(node);
    }
    let nodes = futures::future::join_all(node_futures)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let candidates: HashMap<AccountId, CandidateInfo> = accounts
        .iter()
        .cloned()
        .zip(&nodes)
        .map(|(account, node)| {
            (
                account.id().clone(),
                CandidateInfo {
                    account_id: account.id().to_string().parse().unwrap(),
                    url: node.address.clone(),
                    cipher_pk: node.cipher_pk.to_bytes(),
                    sign_pk: node.sign_pk.to_string().parse().unwrap(),
                },
            )
        })
        .collect();
    ctx.mpc_contract
        .call("init")
        .args_json(json!({
            "threshold": 2,
            "candidates": candidates
        }))
        .transact()
        .await?
        .into_result()?;

    Ok(Nodes::Docker { ctx, nodes })
}

pub async fn host(nodes: usize, docker_client: &DockerClient) -> anyhow::Result<Nodes> {
    let ctx = setup(docker_client).await?;

    let accounts = futures::future::join_all((0..nodes).map(|_| ctx.worker.dev_create_account()))
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let mut node_futures = Vec::with_capacity(nodes);
    for account in accounts.iter().take(nodes) {
        node_futures.push(local::Node::run(&ctx, account.id(), account.secret_key()));
    }
    let nodes = futures::future::join_all(node_futures)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let candidates: HashMap<AccountId, CandidateInfo> = accounts
        .iter()
        .cloned()
        .zip(&nodes)
        .map(|(account, node)| {
            (
                account.id().clone(),
                CandidateInfo {
                    account_id: account.id().to_string().parse().unwrap(),
                    url: node.address.clone(),
                    cipher_pk: node.cipher_pk.to_bytes(),
                    sign_pk: node.account_sk.public_key().to_string().parse().unwrap(),
                },
            )
        })
        .collect();
    ctx.mpc_contract
        .call("init")
        .args_json(json!({
            "threshold": 2,
            "candidates": candidates
        }))
        .transact()
        .await?
        .into_result()?;

    Ok(Nodes::Local { ctx, nodes })
}

pub async fn run(nodes: usize, docker_client: &DockerClient) -> anyhow::Result<Nodes> {
    #[cfg(feature = "docker-test")]
    return docker(nodes, docker_client).await;

    #[cfg(not(feature = "docker-test"))]
    return host(nodes, docker_client).await;
}
