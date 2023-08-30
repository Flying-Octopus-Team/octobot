use graphql_client::GraphQLQuery;
use reqwest::header;
use thiserror::Error;

use crate::{error::Error, SETTINGS};

#[derive(Error, Debug)]
pub enum WikiError {
    #[error("Wiki error: {code:?} {slug:?} {message:?}")]
    Error {
        code: i64,
        slug: String,
        message: Option<String>,
    },
    #[error("Wiki error: {source:?}")]
    ReqwestError {
        #[from]
        source: reqwest::Error,
    },
    #[error("Wiki error: {errors:?}")]
    GraphqlClientError { errors: Vec<graphql_client::Error> },
}

impl From<Vec<graphql_client::Error>> for WikiError {
    fn from(errors: Vec<graphql_client::Error>) -> Self {
        WikiError::GraphqlClientError { errors }
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "wiki/schema.graphql",
    query_path = "wiki/mutations/assign_user_group.graphql",
    response_derives = "Debug"
)]
pub struct AssignUserGroup;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "wiki/schema.graphql",
    query_path = "wiki/mutations/unassign_user_group.graphql",
    response_derives = "Debug"
)]
pub struct UnassignUserGroup;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "wiki/schema.graphql",
    query_path = "wiki/mutations/create_user.graphql",
    response_derives = "Debug,Clone"
)]
pub struct CreateUser;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "wiki/schema.graphql",
    query_path = "wiki/queries/search_user.graphql",
    response_derives = "Debug,Clone"
)]
pub struct SearchUser;

fn get_client() -> Result<reqwest::Client, Error> {
    let mut headers = header::HeaderMap::new();

    let mut auth_value = header::HeaderValue::from_static(&SETTINGS.wiki.token);
    auth_value.set_sensitive(true);
    headers.insert(header::AUTHORIZATION, auth_value);

    let client = reqwest::Client::builder()
        .user_agent("octobot/".to_owned() + env!("CARGO_PKG_VERSION"))
        .default_headers(headers)
        .build()?;

    Ok(client)
}

pub async fn assign_user_group(variables: assign_user_group::Variables) -> Result<(), Error> {
    let client = get_client()?;

    let body = AssignUserGroup::build_query(variables);

    let res = client
        .post(&SETTINGS.wiki.graphql)
        .json(&body)
        .send()
        .await?;

    let response_body: graphql_client::Response<assign_user_group::ResponseData> =
        res.json().await?;

    if let Some(errors) = response_body.errors {
        return Err(Into::<WikiError>::into(errors))?;
    }

    let response_result = response_body
        .data
        .unwrap()
        .groups
        .unwrap()
        .assign_user
        .unwrap()
        .response_result
        .unwrap();

    if response_result.succeeded {
        Ok(())
    } else {
        Err(WikiError::Error {
            code: response_result.error_code,
            slug: response_result.slug,
            message: response_result.message,
        }
        .into())
    }
}

pub async fn unassign_user_group(variables: unassign_user_group::Variables) -> Result<(), Error> {
    let client = get_client()?;

    let body = UnassignUserGroup::build_query(variables);

    let res = client
        .post(&SETTINGS.wiki.graphql)
        .json(&body)
        .send()
        .await?;

    let response_body: graphql_client::Response<unassign_user_group::ResponseData> =
        res.json().await?;

    if let Some(errors) = response_body.errors {
        return Err(Into::<WikiError>::into(errors))?;
    }

    let response_result = response_body
        .data
        .unwrap()
        .groups
        .unwrap()
        .unassign_user
        .unwrap()
        .response_result
        .unwrap();

    if response_result.succeeded {
        Ok(())
    } else {
        Err(WikiError::Error {
            code: response_result.error_code,
            slug: response_result.slug,
            message: response_result.message,
        }
        .into())
    }
}

pub async fn create_user(variables: create_user::Variables) -> Result<Option<i64>, Error> {
    let client = get_client()?;

    let body = CreateUser::build_query(variables);

    let res = client
        .post(&SETTINGS.wiki.graphql)
        .json(&body)
        .send()
        .await?;

    let response_body: graphql_client::Response<create_user::ResponseData> = res.json().await?;

    if let Some(errors) = response_body.errors {
        return Err(Into::<WikiError>::into(errors))?;
    }

    let create_user_users_create = response_body.data.unwrap().users.unwrap().create;

    let response_result = &create_user_users_create.as_ref().unwrap().response_result;

    if response_result.succeeded {
        Ok(create_user_users_create
            .unwrap()
            .user
            .as_ref()
            .map(|user| user.id))
    } else {
        Err(WikiError::Error {
            code: response_result.error_code,
            slug: response_result.slug.clone(),
            message: response_result.message.clone(),
        }
        .into())
    }
}

pub async fn find_user_by_email(email: String) -> Result<Option<i64>, Error> {
    let client = get_client()?;

    let variables = search_user::Variables { query: email };

    let body = SearchUser::build_query(variables);

    let res = client
        .post(&SETTINGS.wiki.graphql)
        .json(&body)
        .send()
        .await?;

    let response_body: graphql_client::Response<search_user::ResponseData> = res.json().await?;

    if let Some(errors) = response_body.errors {
        return Err(Into::<WikiError>::into(errors))?;
    }

    let search_user_users = response_body.data.unwrap().users.unwrap().search.unwrap();

    let len = search_user_users.len();

    if len == 0 {
        Ok(None)
    } else {
        Ok(search_user_users[0].as_ref().map(|user| user.id))
    }
}

pub async fn find_or_create_user(email: String, name: String) -> Result<i64, Error> {
    let user_id = find_user_by_email(email.clone()).await?;

    if let Some(id) = user_id {
        return Ok(id);
    }

    let variables = create_user::Variables {
        email: email.clone(),
        name,
        provider_key: SETTINGS.wiki.provider_key.clone(),
        groups: vec![Some(SETTINGS.wiki.guest_group_id)],
    };

    let user_id = create_user(variables).await?;

    if let Some(id) = user_id {
        Ok(id)
    } else {
        find_user_by_email(email).await.map(|id| id.unwrap())
    }
}
