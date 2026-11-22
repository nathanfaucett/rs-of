pub(crate) mod client_onboarding;
pub(crate) mod identity;
pub(crate) mod native;
pub(crate) mod owner_adapters;
pub(crate) mod replication;
pub(crate) mod timing;

pub(crate) async fn seed_approved_device(
    repo: &impl device_service::DeviceRepo,
    owner: of_model::id::Id,
    name: &str,
    endpoint: &str,
) -> of_model::id::Id {
    let pending = repo
        .create(
            owner.to_string(),
            name.into(),
            endpoint.into(),
            String::new(),
            Vec::new(),
            0,
        )
        .await
        .expect("pending fixture Device");
    if pending.state == device_service::DeviceState::Pending {
        repo.approve_by_admin(pending.id)
            .await
            .expect("fixture approval")
            .expect("pending fixture Device");
    }
    pending.id
}
