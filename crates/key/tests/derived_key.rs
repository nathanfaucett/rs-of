use k256::ecdsa::VerifyingKey as ECDSAVerifyingKey;

use key::error::KeyResult;
use key::master_key::MasterKey;

const TEST_ENTROPY: [u8; 32] = [0x42; 32];

#[test]
fn derive_matches_parent_path_extension() -> KeyResult<()> {
    let master = MasterKey::from_entropy(TEST_ENTROPY)?;

    let _parent = master.derive("m/0/1")?;
    let child = master.derive("m/0/1/2/3")?;

    let direct = master.derive("m/0/1/2/3")?;

    assert_eq!(child.key().private_key(), direct.key().private_key());

    Ok(())
}

#[test]
fn verify_derived_key_accepts_descendant() -> KeyResult<()> {
    let master = MasterKey::from_entropy(TEST_ENTROPY)?;

    let parent = master.derive("m/0/1")?;
    let child = master.derive("m/0/1/2/3")?;

    assert!(parent.verify_derived_key(child)?);

    Ok(())
}

#[test]
fn verify_derived_key_rejects_unrelated_branch() -> KeyResult<()> {
    let master = MasterKey::from_entropy(TEST_ENTROPY)?;

    let parent = master.derive("m/0/1")?;
    let other = master.derive("m/5/6")?;

    assert!(!parent.verify_derived_key(other)?);

    Ok(())
}

#[test]
fn verify_ecdsa_key_accepts_matching_descendant() -> KeyResult<()> {
    let master = MasterKey::from_entropy(TEST_ENTROPY)?;

    let parent = master.derive("m/0/1")?;
    let child = master.derive("m/0/1/2/3")?;

    let verifying_key: ECDSAVerifyingKey = *child.key().private_key().verifying_key();

    assert!(parent.verify_ecdsa_key("m/0/1/2/3", &verifying_key)?);

    Ok(())
}

#[test]
fn verify_ecdsa_key_rejects_non_descendant_path() -> KeyResult<()> {
    let master = MasterKey::from_entropy(TEST_ENTROPY)?;

    let parent = master.derive("m/0/1")?;
    let other = master.derive("m/5/6/7")?;

    let verifying_key: ECDSAVerifyingKey = *other.key().private_key().verifying_key();

    assert!(!parent.verify_ecdsa_key("m/5/6/7", &verifying_key)?);

    Ok(())
}

#[test]
fn verify_ecdsa_key_rejects_hardened_suffix() -> KeyResult<()> {
    let master = MasterKey::from_entropy(TEST_ENTROPY)?;

    let parent = master.derive("m/0/1")?;
    let child = master.derive("m/0/1/2/3")?;

    let verifying_key: ECDSAVerifyingKey = *child.key().private_key().verifying_key();

    assert!(!parent.verify_ecdsa_key("m/0/1/2'/3", &verifying_key)?);

    Ok(())
}

#[test]
fn derivation_path_is_tracked() -> KeyResult<()> {
    let master = MasterKey::from_entropy(TEST_ENTROPY)?;

    let child = master.derive("m/0/1/2")?;

    assert_eq!(child.derivation_path().to_string(), "m/0/1/2");

    Ok(())
}

#[test]
fn display_outputs_derivation_path() -> KeyResult<()> {
    let master = MasterKey::from_entropy(TEST_ENTROPY)?;

    let child = master.derive("m/0/1/2")?;

    assert_eq!(child.to_string(), "m/0/1/2");

    Ok(())
}
