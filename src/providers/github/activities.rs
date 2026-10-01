use super::{comments, events, review_threads};
use crate::domain::{
    activity::Activity,
    comment::{Comment, CommentThread},
    event::TimelineEvent,
    pr::PrId,
};
use crate::providers::error::FetchError;

pub fn fetch(pr_number: PrId) -> Result<Activity, FetchError> {
    timed(pr_number, "activity", || {
        fetch_parts(
            || {
                timed(pr_number, "comments", || {
                    comments::fetch_comments(pr_number)
                })
            },
            || timed(pr_number, "events", || events::fetch_events(pr_number)),
            || {
                timed(pr_number, "review_threads", || {
                    review_threads::fetch_review_threads(pr_number)
                })
            },
        )
    })
}

fn timed<T>(
    pr_number: PrId,
    part: &str,
    fetch: impl FnOnce() -> Result<T, FetchError>,
) -> Result<T, FetchError> {
    let started = std::time::Instant::now();
    let result = fetch();
    tracing::info!(
        pr_number = pr_number.0,
        part,
        elapsed_ms = started.elapsed().as_millis(),
        success = result.is_ok(),
        "GitHub activity fetch"
    );
    result
}

fn fetch_parts(
    comments: impl FnOnce() -> Result<Vec<Comment>, FetchError> + Send,
    events: impl FnOnce() -> Result<Vec<TimelineEvent>, FetchError> + Send,
    threads: impl FnOnce() -> Result<Vec<CommentThread>, FetchError> + Send,
) -> Result<Activity, FetchError> {
    // The caller already runs on a blocking worker. These independent reads can
    // overlap; keep Activity atomic so a failed part never looks like an empty one.
    std::thread::scope(|scope| {
        let events = scope.spawn(events);
        let threads = scope.spawn(threads);
        let comments = comments();
        let events = events
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
        let threads = threads
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
        Ok(Activity {
            comments: comments?,
            events: events?,
            threads: threads?,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{comment::CommentId, event::EventKind, user::User};
    use std::{sync::mpsc, time::Duration};

    #[test]
    fn activity_reads_overlap_and_preserve_all_parts() {
        let (started, starts) = mpsc::channel();
        let (release_comments, comments_gate) = mpsc::channel();
        let (release_events, events_gate) = mpsc::channel();
        let (release_threads, threads_gate) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let await_release = |gate: mpsc::Receiver<()>| {
                started.send(()).unwrap();
                gate.recv_timeout(Duration::from_secs(5)).unwrap();
            };
            fetch_parts(
                || {
                    await_release(comments_gate);
                    Ok(vec![Comment {
                        id: Some(CommentId(7)),
                        author: User {
                            username: "alice".into(),
                        },
                        account: crate::domain::user::AccountKind::Person,
                        content: "Review this".into(),
                        created: chrono::Utc::now(),
                        reactions: vec![],
                        reply_to: None,
                    }])
                },
                || {
                    await_release(events_gate);
                    Ok(vec![TimelineEvent {
                        actor: None,
                        kind: EventKind::Approved,
                        created: chrono::Utc::now(),
                    }])
                },
                || {
                    await_release(threads_gate);
                    Ok(vec![CommentThread {
                        comments: vec![],
                        reply_to: Some(CommentId(9)),
                        anchor: None,
                    }])
                },
            )
        });
        // No request can finish until all three have started. Sequential reads
        // fail this check without relying on elapsed-time performance thresholds.
        for _ in 0..3 {
            starts.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        release_comments.send(()).unwrap();
        release_events.send(()).unwrap();
        release_threads.send(()).unwrap();
        let activity = worker.join().unwrap().unwrap();
        assert_eq!(activity.comments[0].id, Some(CommentId(7)));
        assert_eq!(activity.events[0].kind, EventKind::Approved);
        assert_eq!(activity.threads[0].reply_to, Some(CommentId(9)));
    }

    #[test]
    fn failure_in_any_part_fails_the_whole_activity() {
        for failed in 0..3 {
            let result = fetch_parts(
                || {
                    if failed == 0 {
                        Err(FetchError::Timeout)
                    } else {
                        Ok(vec![])
                    }
                },
                || {
                    if failed == 1 {
                        Err(FetchError::Timeout)
                    } else {
                        Ok(vec![])
                    }
                },
                || {
                    if failed == 2 {
                        Err(FetchError::Timeout)
                    } else {
                        Ok(vec![])
                    }
                },
            );
            assert!(matches!(result, Err(FetchError::Timeout)));
        }
    }
}
