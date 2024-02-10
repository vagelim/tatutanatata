use std::{collections::VecDeque, sync::Arc};

use anyhow::{Context, Result};
use futures::Stream;
use reqwest::{Method, Response};
use serde::de::DeserializeOwned;
use tracing::debug;

use crate::proto::{Base64Url, Entity};

const STREAM_BATCH_SIZE: u64 = 1000;

#[derive(Debug, Clone)]
pub struct Client {
    inner: reqwest::Client,
}

impl Client {
    pub fn try_new() -> Result<Self> {
        let inner = reqwest::Client::builder()
            .build()
            .context("set up HTTPs client")?;
        Ok(Self { inner })
    }

    pub async fn service_request<Req, Resp>(
        &self,
        method: Method,
        path: &str,
        data: &Req,
        access_token: Option<&Base64Url>,
    ) -> Result<Resp>
    where
        Req: serde::Serialize,
        Resp: DeserializeOwned,
    {
        self.do_json(method, "sys", path, data, access_token, &[])
            .await
    }

    pub async fn service_request_tutanota<Req, Resp>(
        &self,
        method: Method,
        path: &str,
        data: &Req,
        access_token: Option<&Base64Url>,
    ) -> Result<Resp>
    where
        Req: serde::Serialize,
        Resp: DeserializeOwned,
    {
        self.do_json(method, "tutanota", path, data, access_token, &[])
            .await
    }

    pub async fn service_request_no_response<Req>(
        &self,
        method: Method,
        path: &str,
        data: &Req,
        access_token: Option<&Base64Url>,
    ) -> Result<Response>
    where
        Req: serde::Serialize,
    {
        self.do_request(method, "sys", path, data, access_token, &[])
            .await
    }

    pub fn stream<Resp>(
        &self,
        path: &str,
        access_token: Option<&Base64Url>,
    ) -> impl Stream<Item = Result<Resp>>
    where
        Resp: DeserializeOwned + Entity,
    {
        let state = StreamState {
            buffer: VecDeque::default(),
            next_start: "------------".to_owned(),
        };
        let path = Arc::new(path.to_owned());
        let access_token = Arc::new(access_token.cloned());
        let this = self.clone();

        futures::stream::try_unfold(state, move |mut state: StreamState<Resp>| {
            let path = Arc::clone(&path);
            let access_token = Arc::clone(&access_token);
            let this = this.clone();
            async move {
                loop {
                    if let Some(next) = state.buffer.pop_front() {
                        return Ok(Some((next, state)));
                    }

                    // buffer empty
                    state.buffer = this
                        .do_json::<(), Vec<Resp>>(
                            Method::GET,
                            "tutanota",
                            &path,
                            &(),
                            access_token.as_ref().as_ref(),
                            &[
                                ("start", &state.next_start),
                                ("count", &STREAM_BATCH_SIZE.to_string()),
                                ("reverse", "false"),
                            ],
                        )
                        .await
                        .context("fetch next page")?
                        .into();
                    match state.buffer.back() {
                        None => {
                            // reached end
                            return Ok(None);
                        }
                        Some(o) => {
                            state.next_start = o.id().to_owned();
                        }
                    }
                }
            }
        })
    }

    async fn do_json<Req, Resp>(
        &self,
        method: Method,
        prefix: &str,
        path: &str,
        data: &Req,
        access_token: Option<&Base64Url>,
        query: &[(&str, &str)],
    ) -> Result<Resp>
    where
        Req: serde::Serialize,
        Resp: DeserializeOwned,
    {
        let resp = self
            .do_request(method, prefix, path, data, access_token, query)
            .await?
            .json::<Resp>()
            .await
            .context("fetch JSON response")?;

        Ok(resp)
    }

    async fn do_request<Req>(
        &self,
        method: Method,
        prefix: &str,
        path: &str,
        data: &Req,
        access_token: Option<&Base64Url>,
        query: &[(&str, &str)],
    ) -> Result<Response>
    where
        Req: serde::Serialize,
    {
        debug!(%method, prefix, path, "service request",);

        let mut req = self
            .inner
            .request(method, format!("https://app.tuta.com/rest/{prefix}/{path}"));

        if let Some(access_token) = access_token {
            req = req.header("accessToken", access_token.to_string());
        }

        let resp = req
            .json(data)
            .query(query)
            .send()
            .await
            .context("initial request")?
            .error_for_status()
            .context("return status")?;

        Ok(resp)
    }
}

struct StreamState<T> {
    buffer: VecDeque<T>,
    next_start: String,
}
