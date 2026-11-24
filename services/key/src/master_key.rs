use core::str::FromStr;

use bip32::{ChildNumber, DerivationPath, XPrv};
use bip39::Mnemonic;
use k256::ecdsa::VerifyingKey as ECDSAVerifyingKey;
use zeroize::Zeroizing;

use crate::derived_key::DerivedKey;
use crate::error::KeyError;
use crate::error::KeyResult;

#[derive(Debug)]
pub struct MasterKey {
    key: XPrv,
}

impl TryFrom<Mnemonic> for MasterKey {
    type Error = KeyError;

    fn try_from(mnemonic: Mnemonic) -> Result<Self, Self::Error> {
        Self::from_mnemonic(mnemonic)
    }
}

impl MasterKey {
    /// Creates a master key from a mnemonic and normalized passphrase.
    ///
    /// # Errors
    ///
    /// Returns `KeyError::Bip32` if the generated seed is not a valid extended private key.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "preserve the owned-mnemonic public API"
    )]
    pub fn from_mnemonic_with_passphrase<S>(
        mnemonic: Mnemonic,
        normalized_passphrase: S,
    ) -> KeyResult<Self>
    where
        S: AsRef<str>,
    {
        let seed = Zeroizing::new(
            mnemonic
                .to_seed_normalized(normalized_passphrase.as_ref())
                .to_vec(),
        );
        let key = XPrv::new(seed.as_slice())?;

        Ok(Self { key })
    }

    /// Creates a master key from entropy and a normalized passphrase.
    ///
    /// # Errors
    ///
    /// Returns `KeyError::Bip39` if `entropy` is invalid, or `KeyError::Bip32` if
    /// master-key creation fails.
    pub fn from_entropy_with_passphrase<T, S>(
        entropy: T,
        normalized_passphrase: S,
    ) -> KeyResult<Self>
    where
        T: AsRef<[u8]>,
        S: AsRef<str>,
    {
        Self::from_mnemonic_with_passphrase(
            Mnemonic::from_entropy(entropy.as_ref())?,
            normalized_passphrase,
        )
    }

    /// Creates a master key from entropy without a passphrase.
    ///
    /// # Errors
    ///
    /// Returns `KeyError::Bip39` if `entropy` is invalid, or `KeyError::Bip32` if
    /// master-key creation fails.
    pub fn from_entropy<T>(entropy: T) -> KeyResult<Self>
    where
        T: AsRef<[u8]>,
    {
        Self::from_entropy_with_passphrase(entropy, "")
    }

    /// Generates 32 bytes of random entropy.
    ///
    /// # Errors
    ///
    /// Returns `KeyError::Random` if randomness generation fails.
    pub fn entropy() -> KeyResult<Zeroizing<[u8; 32]>> {
        let mut entropy = Zeroizing::new([0u8; 32]);
        getrandom::fill(entropy.as_mut())?;
        Ok(entropy)
    }

    /// Creates a master key from a mnemonic without a passphrase.
    ///
    /// # Errors
    ///
    /// Returns `KeyError::Bip32` if the generated seed is not a valid extended private key.
    pub fn from_mnemonic(mnemonic: Mnemonic) -> KeyResult<Self> {
        Self::from_mnemonic_with_passphrase(mnemonic, "")
    }

    /// Parses an extended private master key.
    ///
    /// # Errors
    ///
    /// Returns `KeyError::Bip32` if `xprv` is invalid.
    pub fn from_xprv<S>(xprv: S) -> KeyResult<Self>
    where
        S: AsRef<str>,
    {
        let key = XPrv::from_str(xprv.as_ref())?;
        Ok(Self { key })
    }

    #[must_use]
    pub const fn key(&self) -> &XPrv {
        &self.key
    }

    /// Derives one child private key.
    ///
    /// # Errors
    ///
    /// Returns `KeyError::Bip32` if child key derivation fails.
    pub fn derive_from_child_number(&self, child_number: ChildNumber) -> KeyResult<DerivedKey> {
        let key = self.key.derive_child(child_number)?;

        let mut derivation_path = DerivationPath::default();
        derivation_path.push(child_number);

        Ok(DerivedKey::new(key, derivation_path))
    }

    /// Derives one normal or hardened child private key.
    ///
    /// # Errors
    ///
    /// Returns `KeyError::Bip32` if the child number is invalid or derivation fails.
    pub fn derive_from_index_and_hardened(
        &self,
        index: u32,
        hardened: bool,
    ) -> KeyResult<DerivedKey> {
        self.derive_from_child_number(ChildNumber::new(index, hardened)?)
    }

    /// Derives the private key at `path` from this master key.
    ///
    /// # Errors
    ///
    /// Returns `KeyError::Bip32` if `path` is invalid or child key derivation fails.
    pub fn derive<S>(&self, path: S) -> KeyResult<DerivedKey>
    where
        S: AsRef<str>,
    {
        let derivation_path = DerivationPath::from_str(path.as_ref())?;

        let mut key = self.key.clone();

        for child in derivation_path.as_ref() {
            key = key.derive_child(*child)?;
        }

        Ok(DerivedKey::new(key, derivation_path))
    }

    /// Checks whether `child` matches private derivation along its recorded path.
    ///
    /// # Errors
    ///
    /// Returns `KeyError::Bip32` if child key derivation fails.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "preserve the owned-key public API"
    )]
    pub fn verify_derived_key(&self, child: DerivedKey) -> KeyResult<bool> {
        let mut key = self.key.clone();

        for child in child.derivation_path().as_ref() {
            key = key.derive_child(*child)?;
        }

        Ok(key.private_key() == child.key().private_key())
    }

    /// Checks whether `child` matches public derivation along `path`.
    ///
    /// # Errors
    ///
    /// Returns `KeyError::Bip32` if `path` is invalid.
    pub fn verify_ecdsa_key<S>(&self, path: S, child: &ECDSAVerifyingKey) -> KeyResult<bool>
    where
        S: AsRef<str>,
    {
        let derivation_path = DerivationPath::from_str(path.as_ref())?;

        let mut xpub = self.key.public_key();

        for child in derivation_path.as_ref() {
            if child.is_hardened() {
                return Ok(false);
            }

            match xpub.derive_child(*child) {
                Ok(next) => xpub = next,
                Err(_) => return Ok(false),
            }
        }

        Ok(xpub.public_key() == child)
    }
}

impl From<MasterKey> for DerivedKey {
    fn from(value: MasterKey) -> Self {
        Self::new(value.key, DerivationPath::default())
    }
}
