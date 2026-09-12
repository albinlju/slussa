use super::cli;
use crate::providers::FetchError;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

pub(super) fn pr_nodes<T: DeserializeOwned>(
    pr: u64,
    field: &str,
    selection: &str,
) -> Result<Vec<T>, FetchError> {
    nodes(
        |cursor| {
            format!(
                "query($owner: String!, $name: String!) {{ repository(owner: $owner, name: $name) {{ item: pullRequest(number: {pr}) {{ connection: {field}(first: 100, after: {cursor}) {{ nodes {{ {selection} }} pageInfo {{ hasNextPage endCursor }} }} }} }} }}"
            )
        },
        &["data", "repository", "item", "connection"],
    )
}

pub(super) fn node_nodes<T: DeserializeOwned>(
    id: &str,
    kind: &str,
    field: &str,
    selection: &str,
) -> Result<Vec<T>, FetchError> {
    let id = serde_json::to_string(id).expect("string JSON");
    nodes(
        |cursor| {
            format!(
                "query {{ item: node(id: {id}) {{ ... on {kind} {{ connection: {field}(first: 100, after: {cursor}) {{ nodes {{ {selection} }} pageInfo {{ hasNextPage endCursor }} }} }} }} }}"
            )
        },
        &["data", "item", "connection"],
    )
}

pub(super) fn repo_nodes<T: DeserializeOwned>(
    field: &str,
    selection: &str,
) -> Result<Vec<T>, FetchError> {
    nodes(
        |cursor| {
            format!(
                "query($owner: String!, $name: String!) {{ repository(owner: $owner, name: $name) {{ connection: {field}(first: 100, after: {cursor}) {{ nodes {{ {selection} }} pageInfo {{ hasNextPage endCursor }} }} }} }}"
            )
        },
        &["data", "repository", "connection"],
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

fn nodes<T: DeserializeOwned>(
    query: impl Fn(&str) -> String,
    path: &[&str],
) -> Result<Vec<T>, FetchError> {
    collect(|cursor| {
        let cursor = serde_json::to_string(&cursor).expect("cursor JSON");
        let query = query(&cursor);
        let mut args = vec!["api", "graphql"];
        if query.contains("$owner") {
            args.extend(["-F", "owner={owner}", "-F", "name={repo}"]);
        }
        args.extend(["-f", &query]);
        let query_arg = format!("query={query}");
        *args.last_mut().expect("query argument") = &query_arg;
        let mut value: Value = cli::run_gh_json(&args)?;
        if value.get("errors").is_some() {
            return Err(FetchError::InvalidInput(
                "GitHub returned an incomplete GraphQL response.".into(),
            ));
        }
        for key in path {
            value = value
                .get_mut(*key)
                .map(Value::take)
                .ok_or_else(|| FetchError::ParseFailed(format!("Missing {key}")))?;
        }
        let info = serde_json::from_value(value["pageInfo"].take())
            .map_err(|e| FetchError::ParseFailed(e.to_string()))?;
        let items = serde_json::from_value(value["nodes"].take())
            .map_err(|e| FetchError::ParseFailed(e.to_string()))?;
        Ok((items, info))
    })
}

fn collect<T>(
    mut fetch: impl FnMut(Option<&str>) -> Result<(Vec<T>, PageInfo), FetchError>,
) -> Result<Vec<T>, FetchError> {
    let mut cursor: Option<String> = None;
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    loop {
        let (nodes, info) = fetch(cursor.as_deref())?;
        result.extend(nodes);
        if !info.has_next_page {
            return Ok(result);
        }
        let next = info
            .end_cursor
            .filter(|c| !c.is_empty() && seen.insert(c.clone()))
            .ok_or_else(|| FetchError::ParseFailed("Missing or repeated GraphQL cursor".into()))?;
        cursor = Some(next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn follows_cursors_and_fails_instead_of_returning_partial_data() {
        let result = collect(|cursor| {
            Ok(match cursor {
                None => (
                    vec![1],
                    PageInfo {
                        has_next_page: true,
                        end_cursor: Some("next".into()),
                    },
                ),
                Some("next") => (
                    vec![2],
                    PageInfo {
                        has_next_page: false,
                        end_cursor: None,
                    },
                ),
                _ => panic!(),
            })
        })
        .unwrap();
        assert_eq!(result, vec![1, 2]);
        assert!(
            collect(|_| Ok((
                vec![1],
                PageInfo {
                    has_next_page: true,
                    end_cursor: Some("same".into())
                }
            )))
            .is_err()
        );
        assert!(collect::<u8>(|_| Err(FetchError::Network("offline".into()))).is_err());
    }
}
