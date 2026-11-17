use key::error::KeyResult;
use key::master_key::MasterKey;

const TEST_ENTROPY: [u8; 32] = [0x42; 32];

#[test]
fn derive_child_number_matches_path_derivation() -> KeyResult<()> {
    let master = MasterKey::from_entropy(TEST_ENTROPY)?;

    let child_a = master.derive_from_index_and_hardened(0, false)?;
    let child_b = master.derive("m/0")?;

    assert_eq!(child_a.key().private_key(), child_b.key().private_key());

    Ok(())
}

#[test]
fn derive_nested_path_matches_stepwise_derivation() -> KeyResult<()> {
    let master = MasterKey::from_entropy(TEST_ENTROPY)?;

    let child = master.derive("m/0")?.derive("m/0/1")?.derive("m/0/1/2")?;

    let direct = master.derive("m/0/1/2")?;

    assert_eq!(child.key().private_key(), direct.key().private_key());

    Ok(())
}

#[test]
fn verify_derived_key_accepts_valid_child() -> KeyResult<()> {
    let master = MasterKey::from_entropy(TEST_ENTROPY)?;
    let child = master.derive("m/0/1/2")?;

    assert!(master.verify_derived_key(child)?);

    Ok(())
}

#[test]
fn verify_derived_key_rejects_unrelated_child() -> KeyResult<()> {
    let master1 = MasterKey::from_entropy(TEST_ENTROPY)?;
    let master2 = MasterKey::from_entropy([0x99; 32])?;

    let child = master2.derive("m/0/1/2")?;

    assert!(!master1.verify_derived_key(child)?);

    Ok(())
}
