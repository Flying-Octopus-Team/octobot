use crate::discord::Error;
use graphql_client::GraphQLQuery;
use reqwest::header;

use crate::SETTINGS;

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

    if response_body.errors.is_some() {
        return Err(anyhow!(response_body.errors.unwrap()[0].message.clone()));
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
        Err(anyhow!(response_result.message.unwrap()))
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

    if response_body.errors.is_some() {
        return Err(anyhow!(response_body.errors.unwrap()[0].message.clone()));
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
        Err(anyhow!(response_result.message.unwrap()))
    }
}

pub async fn create_user(variables: create_user::Variables) -> Result<i64, Error> {
    let client = get_client()?;

    let body = CreateUser::build_query(variables);

    let res = client
        .post(&SETTINGS.wiki.graphql)
        .json(&body)
        .send()
        .await?;

    let response_body: graphql_client::Response<create_user::ResponseData> = res.json().await?;

    if response_body.errors.is_some() {
        return Err(anyhow!(response_body.errors.unwrap()[0].message.clone()));
    }

    let create_user_users_create = response_body.data.unwrap().users.unwrap().create;

    let response_result = &create_user_users_create.as_ref().unwrap().response_result;

    if response_result.succeeded {
        Ok(create_user_users_create.unwrap().user.unwrap().id)
    } else {
        Err(anyhow!(response_result.message.clone().unwrap()))
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

    if response_body.errors.is_some() {
        return Err(anyhow!(response_body.errors.unwrap()[0].message.clone()));
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

    if user_id.is_some() {
        return Ok(user_id.unwrap());
    }

    let variables = create_user::Variables {
        email,
        name,
        groups: vec![Some(SETTINGS.wiki.member_group_id)],
    };

    create_user(variables).await
}
