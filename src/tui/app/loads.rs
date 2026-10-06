use crate::{
    domain::pr::PrId,
    tui::app::{
        App,
        effect::{Read, WriteError},
        store::{LoadState, Notice, Operation, PrData, WriteTicket},
    },
};

impl App {
    /// Take in a read: settle its bookkeeping, then store what it brought.
    pub(super) fn apply_read(&mut self, read: Read) {
        let key = read.key();
        let failure = read.failure();
        if let Some(error) = failure {
            tracing::warn!("read failed: {key:?}: {error}");
        } else {
            tracing::debug!("read done: {key:?}");
        }
        // A failed read of an older page says so in its own notice and
        // is not a stale list.
        let older_page = matches!(
            &read,
            Read::Prs { group, after: Some(_), .. } if group.is_paged()
        );
        if failure.is_none() {
            self.state.store.refresh_failures.remove(&key);
        } else if !older_page && self.state.store.has_cached_data(&key) {
            self.state.store.refresh_failures.insert(key.clone());
        }
        self.state.store.fetches.remove(&key);
        match read {
            Read::Prs {
                group,
                after,
                result,
            } => self.prs_loaded(group, after.is_some(), result),
            Read::Pr(pr_id, result) => self.requested_pr_read(pr_id, result),
            Read::Commits(pr_id, result) => {
                let result = result.map(|commits| self.state.store.judged_commits(commits));
                if let Ok(new) = &result {
                    let old = self
                        .state
                        .store
                        .cache
                        .details
                        .get(&pr_id)
                        .and_then(|data| data.commits.loaded())
                        .map_or(&[][..], Vec::as_slice);
                    self.state.ui.detail.reconcile_commits(pr_id, old, new);
                }
                self.pr_data_mut(pr_id).commits.reload(result);
            }
            Read::Diff(pr_id, result) => self.pr_data_mut(pr_id).diff.reload(result),
            Read::Builds(pr_id, result) => self.pr_data_mut(pr_id).builds.reload(result),
            Read::Activity(pr_id, result) => {
                let result = result.map(|activity| self.state.store.judged(activity));
                self.pr_data_mut(pr_id).activity.reload(result);
            }
            Read::Mergeability(pr_id, result) => {
                self.pr_data_mut(pr_id).mergeability.reload(result);
            }
            Read::Info(pr_id, result) => self.pr_data_mut(pr_id).info.reload(result),
            Read::CommitDiff(pr_id, oid, result) => {
                self.pr_data_mut(pr_id)
                    .commit_diffs
                    .insert(oid, LoadState::from_result(result));
            }
            Read::RangeDiff(pr_id, range, result) => {
                self.pr_data_mut(pr_id)
                    .range_diffs
                    .insert(range, LoadState::from_result(result));
            }
            Read::BuildLog(pr_id, job, result) => {
                self.pr_data_mut(pr_id)
                    .build_logs
                    .insert(job, LoadState::from_result(result));
            }
        }
        if self.state.store.reload_after_fetch.remove(&key) {
            self.load_resource(key);
        }
    }

    /// Take in the end of a write. The ticket says which PR and operation it was.
    pub(super) fn apply_write(&mut self, ticket: &WriteTicket, result: Result<(), WriteError>) {
        let (pr_id, operation) = (ticket.pr_id(), ticket.operation());
        match &result {
            Ok(()) => tracing::debug!("write done: {operation:?} pr={pr_id}"),
            Err(error) => tracing::warn!("write failed: {operation:?} pr={pr_id}: {error}"),
        }
        self.state.store.operations.remove(&pr_id);
        if operation.sends_editor_text() {
            self.state
                .ui
                .detail
                .submission_finished(pr_id, result.is_ok());
        }
        match result {
            Ok(()) => {
                let notice = Notice::info(format!("PR #{pr_id} · {}", operation.done_label()));
                self.write_done(pr_id, operation, notice);
            }
            // A merge whose branch stayed is done, and says what did not happen.
            Err(WriteError::BranchDeleteFailed(error)) => {
                let notice = Notice::error(format!(
                    "PR #{pr_id} · merged, but deleting the branch failed: {}",
                    error.user_message()
                ));
                self.write_done(pr_id, operation, notice);
            }
            Err(error) => self.write_failed(pr_id, operation, error),
        }
    }

    fn write_done(&mut self, pr_id: PrId, operation: Operation, notice: Notice) {
        self.state.store.notice = Some(notice);
        self.state.store.uncertain_submissions.remove(&pr_id);
        self.state.store.errors.remove(&pr_id);
        // Show the new status at once; the refetch below confirms it.
        // Without this the PR would vanish from the list until the
        // group it moved to is read.
        if let Some(status) = operation.moves_pr_to()
            && let LoadState::Loaded(prs) = &mut self.state.store.cache.prs
            && let Some(pr) = prs.iter_mut().find(|pr| pr.id == pr_id)
        {
            pr.status = status;
        }
        if operation == Operation::RerunBuilds {
            self.reload_builds(pr_id);
        }
        if operation == Operation::Review {
            self.state.store.reviews.remove(&pr_id);
        }
        self.reload_after_mutation(pr_id);
    }

    fn write_failed(&mut self, pr_id: PrId, operation: Operation, error: WriteError) {
        // A write that never left, or that the server refused, changed
        // nothing; any other failure leaves the outcome open.
        if error.may_have_reached_server() {
            self.state.store.uncertain_submissions.insert(pr_id);
            // Some of the runs may have started.
            if operation == Operation::RerunBuilds {
                self.reload_builds(pr_id);
            }
        }
        self.state.store.errors.insert(pr_id, error.user_message());
        if let WriteError::PartialReview {
            posted_comments,
            submitted_summary,
            ..
        } = error
        {
            // What arrived is not sent again, and is read back.
            if let Some(review) = self.state.store.reviews.get_mut(&pr_id) {
                let count = posted_comments.min(review.comments.len());
                review.comments.drain(..count);
                if submitted_summary.is_some() {
                    review.submitted_summary = submitted_summary;
                }
            }
            self.reload_after_mutation(pr_id);
        }
    }

    fn pr_data_mut(&mut self, pr_id: PrId) -> &mut PrData {
        self.state.store.cache.details.entry(pr_id).or_default()
    }
}
