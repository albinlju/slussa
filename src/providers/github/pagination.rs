use super::{cli, graphql};
use crate::providers::FetchError;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

pub(super) fn pr_nodes<T: DeserializeOwned>(
    pr: u64,
    field: &str,
    selection: &str,
) -> Result<Vec<T>, FetchError> {
    let pr = pr.to_string();
    nodes(
        |cursor| {
            graphql::fill(
                graphql::PR_CONNECTION,
                &[
                    ("pr", &pr),
                    ("field", field),
                    ("cursor", cursor),
                    ("selection", selection),
                ],
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
    node_nodes_after(id, kind, field, selection, None)
}

pub(super) fn node_nodes_after<T: DeserializeOwned>(
    id: &str,
    kind: &str,
    field: &str,
    selection: &str,
    cursor: Option<&str>,
) -> Result<Vec<T>, FetchError> {
    let id = Value::String(id.to_owned()).to_string();
    nodes_after(
        |cursor| {
            graphql::fill(
                graphql::NODE_CONNECTION,
                &[
                    ("id", &id),
                    ("kind", kind),
                    ("field", field),
                    ("cursor", cursor),
                    ("selection", selection),
                ],
            )
        },
        &["data", "item", "connection"],
        cursor,
    )
}

pub(super) fn repo_nodes<T: DeserializeOwned>(
    field: &str,
    selection: &str,
) -> Result<Vec<T>, FetchError> {
    nodes(
        |cursor| {
            graphql::fill(
                graphql::REPO_CONNECTION,
                &[
                    ("field", field),
                    ("cursor", cursor),
                    ("selection", selection),
                ],
            )
        },
        &["data", "repository", "connection"],
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PageInfo {
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}

fn nodes<T: DeserializeOwned>(
    query: impl Fn(&str) -> String,
    path: &[&str],
) -> Result<Vec<T>, FetchError> {
    nodes_after(query, path, None)
}

fn nodes_after<T: DeserializeOwned>(
    query: impl Fn(&str) -> String,
    path: &[&str],
    cursor: Option<&str>,
) -> Result<Vec<T>, FetchError> {
    collect_from(cursor, |cursor| {
        let cursor = serde_json::json!(cursor).to_string();
        let query = graphql::compact(&query(&cursor));
        let query_arg = format!("query={query}");
        let mut args = vec!["api", "graphql"];
        if query.contains("$owner") {
            args.extend(["-F", "owner={owner}", "-F", "name={repo}"]);
        }
        args.extend(["-f", &query_arg]);
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

#[cfg(test)]
fn collect<T>(
    fetch: impl FnMut(Option<&str>) -> Result<(Vec<T>, PageInfo), FetchError>,
) -> Result<Vec<T>, FetchError> {
    collect_from(None, fetch)
}

fn collect_from<T>(
    cursor: Option<&str>,
    mut fetch: impl FnMut(Option<&str>) -> Result<(Vec<T>, PageInfo), FetchError>,
) -> Result<Vec<T>, FetchError> {
    let mut cursor = cursor.map(str::to_owned);
    let mut seen = std::collections::HashSet::new();
    if let Some(cursor) = &cursor {
        seen.insert(cursor.clone());
    }
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
    fn continuation_starts_after_embedded_page_and_rejects_repeated_cursor() {
        let mut cursors = Vec::new();
        let items = collect_from(Some("embedded"), |cursor| {
            cursors.push(cursor.unwrap().to_owned());
            Ok(if cursor == Some("embedded") {
                (
                    vec![2],
                    PageInfo {
                        has_next_page: true,
                        end_cursor: Some("second".into()),
                    },
                )
            } else {
                (
                    vec![3],
                    PageInfo {
                        has_next_page: false,
                        end_cursor: None,
                    },
                )
            })
        })
        .unwrap();
        assert_eq!(items, vec![2, 3]);
        assert_eq!(cursors, vec!["embedded", "second"]);
        assert!(
            collect_from(Some("embedded"), |_| Ok((
                vec![1],
                PageInfo {
                    has_next_page: true,
                    end_cursor: Some("embedded".into()),
                }
            )))
            .is_err()
        );
    }

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
