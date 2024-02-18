use std::ops::Deref;

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

use crate::proto::{binary::Base64Url, enums::KdfVersion, keys::Key};

#[derive(Debug, Clone, Copy)]
pub(crate) struct UserPassphraseKey(Key);

impl AsRef<Key> for UserPassphraseKey {
    fn as_ref(&self) -> &Key {
        &self.0
    }
}

impl Deref for UserPassphraseKey {
    type Target = Key;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub(crate) fn derive_passkey(
    kdf_version: KdfVersion,
    passphrase: &str,
    salt: &[u8],
) -> Result<UserPassphraseKey> {
    match kdf_version {
        KdfVersion::Bcrypt => {
            let mut hasher = Sha256::new();
            hasher.update(passphrase.as_bytes());
            let passphrase = hasher.finalize();

            let salt: [u8; 16] = salt.try_into().context("salt length")?;

            let hashed = bcrypt::bcrypt(8, salt, &passphrase);

            Ok(UserPassphraseKey(Key::Aes128(
                hashed[..16].try_into().expect("checked length"),
            )))
        }
        KdfVersion::Argon2id => bail!("not implemented: Argon2id"),
    }
}

pub(crate) fn encode_auth_verifier(passkey: &UserPassphraseKey) -> Base64Url {
    let mut hasher = Sha256::new();
    hasher.update(passkey.0);
    let hashed = hasher.finalize().to_vec();

    Base64Url::from(hashed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_auth_verifier() {
        let pk = derive_passkey(KdfVersion::Bcrypt, "password", b"saltsaltsaltsalt").unwrap();
        let verifier = encode_auth_verifier(&pk);
        assert_eq!(
            verifier.to_string(),
            "r3YdONamUCQ7yFZwPFX8KLWZ4kKnAZLyt7rwi1DCE1I",
        );
    }
}
