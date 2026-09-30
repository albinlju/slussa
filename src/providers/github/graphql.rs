//! GraphQL text for the GitHub provider.
//!
//! Queries are written here as readable multi-line GraphQL with `<<name>>`
//! placeholders, and compacted to a single line when they are sent, so the
//! wire format does not depend on how the source is laid out. Compacting
//! collapses every run of whitespace, which is safe because nothing sent
//! contains a string value with meaningful spacing (ids and cursors have none).

/// Collapse all whitespace runs to single spaces and trim the ends.
pub(super) fn compact(query: &str) -> String {
    query.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Replace each `<<name>>` in `template` with its value.
pub(super) fn fill(template: &str, values: &[(&str, &str)]) -> String {
    values
        .iter()
        .fold(template.to_owned(), |text, (name, value)| {
            text.replace(&format!("<<{name}>>"), value)
        })
}

/// A page of a connection on a pull request of this repository.
pub(super) const PR_CONNECTION: &str = r"
query($owner: String!, $name: String!) {
  repository(owner: $owner, name: $name) {
    item: pullRequest(number: <<pr>>) {
      connection: <<field>>(first: 100, after: <<cursor>>) {
        nodes { <<selection>> }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}";

/// A page of a connection on any object, addressed by its node id.
pub(super) const NODE_CONNECTION: &str = r"
query {
  item: node(id: <<id>>) {
    ... on <<kind>> {
      connection: <<field>>(first: 100, after: <<cursor>>) {
        nodes { <<selection>> }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}";

/// A page of a connection on this repository. `<<args>>` is either empty or
/// arguments ending in a comma and a space, such as `states: OPEN, `.
pub(super) const REPO_CONNECTION: &str = r"
query($owner: String!, $name: String!) {
  repository(owner: $owner, name: $name) {
    connection: <<field>>(<<args>>first: <<first>>, after: <<cursor>>) {
      nodes { <<selection>> }
      pageInfo { hasNextPage endCursor }
    }
  }
}";

/// What decides whether a pull request can be merged.
pub(super) const MERGEABILITY: &str = r"
query($owner: String!, $name: String!, $pr: Int!) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $pr) { mergeable mergeStateStatus reviewDecision }
  }
}";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_collapses_layout_and_nothing_else() {
        assert_eq!(compact("\n  a {\n    b  c\n  }\n"), "a { b c }");
        assert_eq!(compact("already compact"), "already compact");
        assert_eq!(compact(""), "");
    }

    #[test]
    fn fill_replaces_every_occurrence_and_leaves_no_placeholder() {
        let values = [
            ("pr", "7"),
            ("field", "comments"),
            ("cursor", "null"),
            ("selection", "id"),
            ("id", "\"X\""),
            ("kind", "PullRequest"),
            ("args", ""),
            ("first", "100"),
        ];
        for template in [PR_CONNECTION, NODE_CONNECTION, REPO_CONNECTION] {
            assert!(!fill(template, &values).contains("<<"), "{template}");
        }
        assert_eq!(fill("<<a>> and <<a>>", &[("a", "x")]), "x and x");
    }

    /// The layout change must not change what is sent: these are the
    /// single-line queries the provider sent before the templates existed.
    #[test]
    fn templates_compact_to_the_queries_sent_before() {
        let pr = fill(
            PR_CONNECTION,
            &[
                ("pr", "7"),
                ("field", "comments"),
                ("cursor", "null"),
                ("selection", "id"),
            ],
        );
        assert_eq!(
            compact(&pr),
            "query($owner: String!, $name: String!) { repository(owner: $owner, name: $name) { item: pullRequest(number: 7) { connection: comments(first: 100, after: null) { nodes { id } pageInfo { hasNextPage endCursor } } } } }"
        );
        let node = fill(
            NODE_CONNECTION,
            &[
                ("id", "\"PR_1\""),
                ("kind", "PullRequest"),
                ("field", "labels"),
                ("cursor", "null"),
                ("selection", "name"),
            ],
        );
        assert_eq!(
            compact(&node),
            "query { item: node(id: \"PR_1\") { ... on PullRequest { connection: labels(first: 100, after: null) { nodes { name } pageInfo { hasNextPage endCursor } } } } }"
        );
        let repo = fill(
            REPO_CONNECTION,
            &[
                ("field", "pullRequests"),
                ("args", ""),
                ("first", "100"),
                ("cursor", "\"c1\""),
                ("selection", "id"),
            ],
        );
        assert_eq!(
            compact(&repo),
            "query($owner: String!, $name: String!) { repository(owner: $owner, name: $name) { connection: pullRequests(first: 100, after: \"c1\") { nodes { id } pageInfo { hasNextPage endCursor } } } }"
        );
        assert_eq!(
            compact(MERGEABILITY),
            "query($owner: String!, $name: String!, $pr: Int!) { repository(owner: $owner, name: $name) { pullRequest(number: $pr) { mergeable mergeStateStatus reviewDecision } } }"
        );
    }
}
