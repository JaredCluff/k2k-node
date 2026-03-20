use anyhow::{Context, Result};
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::RsaPrivateKey;
use std::fs;
use std::path::Path;

pub struct KeyManager {
    private_key: RsaPrivateKey,
    public_key_pem: String,
    private_key_pem: String,
}

impl KeyManager {
    /// Load or generate the node's RSA key pair.
    pub fn load_or_generate(keys_dir: &str) -> Result<Self> {
        fs::create_dir_all(keys_dir)?;

        let private_path = format!("{}/k2k_private_key.pem", keys_dir);
        let public_path = format!("{}/k2k_public_key.pem", keys_dir);

        if Path::new(&private_path).exists() && Path::new(&public_path).exists() {
            let private_pem = fs::read_to_string(&private_path)
                .context("Failed to read private key")?;
            let public_pem = fs::read_to_string(&public_path)
                .context("Failed to read public key")?;
            let private_key = RsaPrivateKey::from_pkcs8_pem(&private_pem)
                .context("Failed to parse private key")?;

            tracing::info!("Loaded existing RSA key pair from {}", keys_dir);
            Ok(Self {
                private_key,
                public_key_pem: public_pem,
                private_key_pem: private_pem,
            })
        } else {
            tracing::info!("Generating new RSA-2048 key pair...");
            let private_key = RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048)?;
            let public_key = private_key.to_public_key();

            let private_pem = private_key.to_pkcs8_pem(LineEnding::LF)?.to_string();
            let public_pem = public_key.to_public_key_pem(LineEnding::LF)?;

            fs::write(&private_path, &private_pem)?;
            fs::write(&public_path, &public_pem)?;

            // Set permissions on private key (Unix only)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&private_path, fs::Permissions::from_mode(0o600))?;
            }

            tracing::info!("Generated and saved RSA key pair to {}", keys_dir);
            Ok(Self {
                private_key,
                public_key_pem: public_pem,
                private_key_pem: private_pem,
            })
        }
    }

    pub fn public_key_pem(&self) -> &str {
        &self.public_key_pem
    }

    pub fn private_key_pem(&self) -> &str {
        &self.private_key_pem
    }

    pub fn private_key(&self) -> &RsaPrivateKey {
        &self.private_key
    }

    /// Verify a JWT signature against a client's registered public key.
    pub fn verify_jwt(&self, token: &str, public_key_pem: &str) -> Result<k2k_common::K2KClaims> {
        k2k_common::verify_k2k_jwt(token, public_key_pem)
    }
}
