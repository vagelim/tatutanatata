use anyhow::{bail, Context, Result};
use clap::Parser;
use reqwest::Method;
use sha2::{Digest, Sha256};
use tracing::debug;

use crate::{
    client::Client,
    crypto::auth::build_auth_verifier,
    non_empty_string::NonEmptyString,
    proto::{
        Base64Url, SaltServiceRequest, SaltServiceResponse, SessionServiceRequest,
        SessionServiceResponse, UserResponse,
    },
};

/// Login CLI config.
#[derive(Debug, Parser)]
pub struct LoginCLIConfig {
    /// Username
    #[clap(long, env = "TUTANOTA_CLI_USERNAME")]
    username: NonEmptyString,

    /// Password
    #[clap(long, env = "TUTANOTA_CLI_PASSWORD")]
    password: NonEmptyString,
}

/// User session
#[derive(Debug)]
pub struct Session {
    pub user_id: String,
    pub access_token: Base64Url,
}

impl Session {
    /// Perform tutanota login.
    pub async fn login(config: LoginCLIConfig, client: &Client) -> Result<Self> {
        debug!("perform login");

        let req = SaltServiceRequest {
            format: Default::default(),
            mail_address: config.username.to_string(),
        };
        let resp: SaltServiceResponse = client
            .service_requst(Method::GET, "saltservice", &req, None)
            .await
            .context("get salt")?;

        let auth_verifier =
            build_auth_verifier(resp.kdf_version, &config.password, resp.salt.as_ref())
                .context("build auth verifier")?;

        let req = SessionServiceRequest {
            format: Default::default(),
            access_key: Default::default(),
            auth_token: Default::default(),
            auth_verifier,
            client_identifier: env!("CARGO_PKG_NAME").to_owned(),
            mail_address: config.username.to_string(),
            recover_code_verifier: Default::default(),
            user: Default::default(),
        };
        let resp: SessionServiceResponse = client
            .service_requst(Method::POST, "sessionservice", &req, None)
            .await
            .context("get session")?;

        debug!(user = resp.user.as_str(), "got user");

        if !resp.challenges.is_empty() {
            bail!("not implemented: challenges");
        }

        Ok(Self {
            user_id: resp.user,
            access_token: resp.access_token,
        })
    }

    pub async fn logout(self, client: &Client) -> Result<()> {
        let resp: UserResponse = client
            .service_requst(
                Method::GET,
                &format!("user/{}", self.user_id),
                &(),
                Some(&self.access_token),
            )
            .await
            .context("get user")?;

        client
            .service_requst_no_response(
                Method::DELETE,
                &format!(
                    "session/{}/{}",
                    resp.auth.sessions,
                    // session_list_id(&self.access_token),
                    session_element_id(&self.access_token)
                ),
                &(),
                Some(&self.access_token),
            )
            .await
            .context("session deletion")?;

        Ok(())
    }
}

const GENERATE_ID_BYTES_LENGTH: usize = 9;

fn session_element_id(access_token: &Base64Url) -> Base64Url {
    let mut hasher = Sha256::new();
    hasher.update(&access_token.as_ref()[GENERATE_ID_BYTES_LENGTH..]);
    hasher.finalize().to_vec().into()
}

#[allow(dead_code)]
fn session_list_id(access_token: &Base64Url) -> Base64Url {
    access_token.as_ref()[..GENERATE_ID_BYTES_LENGTH].into()
}
