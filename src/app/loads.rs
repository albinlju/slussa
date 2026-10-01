use crate::{
    app::{
        App,
        action::{Read, WriteError},
        navigation::Screen,
        store::{LoadState, Notice, OpenChain, Operation, PrData, WriteTicket},
    },
    domain::pr::{PrBatch, PrGroup, PrId, PullRequest},
    providers::FetchError,
    tui::screens::pr_list::ListContext,
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
            Read::Commits(pr_id, result) => {
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
            Read::Activity(pr_id, result) => self.pr_data_mut(pr_id).activity.reload(result),
            Read::Mergeability(pr_id, result) => {
                self.pr_data_mut(pr_id).mergeability.reload(result);
            }
            Read::Info(pr_id, result) => self.pr_data_mut(pr_id).info.reload(result),
            Read::CommitDiff(pr_id, oid, result) => {
                self.pr_data_mut(pr_id)
                    .commit_diffs
                    .insert(oid, LoadState::from_result(result));
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
                self.state.store.notice = Some(Notice::info(format!(
                    "PR #{pr_id} · {}",
                    operation.done_label()
                )));
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
                if operation == Operation::Review {
                    self.state.store.reviews.remove(&pr_id);
                }
                self.reload_after_mutation(pr_id);
            }
            Err(error) => {
                // A write that never left, or that the server refused, changed
                // nothing; any other failure leaves the outcome open.
                if error.may_have_reached_server() {
                    self.state.store.uncertain_submissions.insert(pr_id);
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
        }
    }

    fn prs_loaded(
        &mut self,
        group: PrGroup,
        continuation: bool,
        result: Result<PrBatch, FetchError>,
    ) {
        let older = continuation && group.is_paged();
        // The highlighted PR keeps the highlight wherever its row ends up.
        let selected_id = self
            .list_rows()
            .get(self.state.ui.list.selected)
            .map(|pr| pr.id);
        if group == PrGroup::Open && result.is_err() {
            self.state.store.open_chain = OpenChain::Idle;
        }
        match result {
            Ok(batch) if group == PrGroup::Open => self.adopt_open_page(continuation, batch),
            Ok(batch) => self.adopt_group(group, older, batch),
            Err(error) if older => {
                self.state.store.notice = Some(Notice::error(format!(
                    "Couldn't load older PRs: {}",
                    error.user_message()
                )));
            }
            // The open group is the list itself: a first failure shows as a
            // failed list, a failed refresh keeps what is there.
            Err(error) if group == PrGroup::Open => {
                self.state.store.cache.prs.reload(Err(error));
            }
            // A closed group never read before has no data to keep, so say so.
            Err(error) if !self.state.store.group_loaded(group) => {
                self.state.store.notice = Some(Notice::error(format!(
                    "Couldn't load {} PRs: {}",
                    group.label(),
                    error.user_message()
                )));
            }
            // A failed refresh of a group already read is recorded in
            // `refresh_failures` above.
            Err(_) => {}
        }
        let filtered = self.list_rows();
        let selected = selected_id
            .and_then(|id| filtered.iter().position(|pr| pr.id == id))
            .unwrap_or_else(|| {
                self.state
                    .ui
                    .list
                    .selected
                    .min(filtered.len().saturating_sub(1))
            });
        self.state.ui.list.selected = selected;
    }

    /// The list's rows as it shows them now.
    fn list_rows(&self) -> Vec<&PullRequest> {
        let state = &self.state;
        let ctx = ListContext::from_store(&state.store, state.ui.list.filter, state.screen);
        state.ui.list.filtered_prs(&ctx)
    }

    /// Take a group's read into the list. A first read replaces the group's
    /// PRs; older closed PRs already loaded stay, and so does the position
    /// reached, or a refresh every minute would throw them away. An older page
    /// goes after what is there. The PR open in the detail screen is never
    /// dropped, so a merge by someone else does not blank the screen.
    /// One page of the open group. A first reading shows each page as it
    /// arrives, in arrival order. A refresh holds the pages and swaps them in
    /// after the last one. Either way the next page is asked for until there is
    /// none or the limit is reached, and then the list takes its order.
    fn adopt_open_page(&mut self, continuation: bool, batch: PrBatch) {
        let more = batch.more.clone();
        let store = &mut self.state.store;
        let chain = match (continuation, std::mem::take(&mut store.open_chain)) {
            (false, _) if matches!(store.cache.prs, LoadState::Loaded(_)) => {
                OpenChain::Collecting(batch.prs)
            }
            (false, _) => {
                self.adopt_group(PrGroup::Open, false, batch);
                OpenChain::Appending
            }
            (true, OpenChain::Appending) => {
                if let LoadState::Loaded(prs) = &mut store.cache.prs {
                    for pr in batch.prs {
                        if !prs.iter().any(|known| known.id == pr.id) {
                            prs.push(pr);
                        }
                    }
                    prs.sort_by_key(|pr| PrGroup::of(&pr.status));
                }
                OpenChain::Appending
            }
            (true, OpenChain::Collecting(mut held)) => {
                held.extend(batch.prs);
                OpenChain::Collecting(held)
            }
            // A page that belongs to a reading that has already ended.
            (true, OpenChain::Idle) => return,
        };
        let store = &mut self.state.store;
        let read = match &chain {
            OpenChain::Collecting(held) => held.len(),
            _ => match &store.cache.prs {
                LoadState::Loaded(prs) => prs
                    .iter()
                    .filter(|pr| PrGroup::of(&pr.status) == PrGroup::Open)
                    .count(),
                _ => 0,
            },
        };
        let next = more.clone().filter(|_| read < store.open_limit());
        // Where `L` continues from, if the reading stops with more left.
        store
            .groups
            .entry(PrGroup::Open)
            .or_default()
            .more
            .clone_from(&more);
        if let Some(after) = next {
            self.state.store.open_chain = chain;
            self.spawn_load_prs(PrGroup::Open, Some(after));
            return;
        }
        if let OpenChain::Collecting(held) = chain {
            self.adopt_group(PrGroup::Open, false, PrBatch { prs: held, more });
        }
    }

    fn adopt_group(&mut self, group: PrGroup, older: bool, batch: PrBatch) {
        let viewing = match self.state.screen {
            Screen::Detail { pr_id, .. } => Some(pr_id),
            Screen::List => None,
        };
        let store = &mut self.state.store;
        let existing = match &mut store.cache.prs {
            LoadState::Loaded(prs) => std::mem::take(prs),
            _ => Vec::new(),
        };
        let state = store.groups.entry(group).or_default();
        let mut notice = None;
        let mut prs = if older {
            let mut prs = existing;
            let mut added = 0;
            for pr in batch.prs {
                if !prs.iter().any(|known| known.id == pr.id) {
                    prs.push(pr);
                    added += 1;
                }
            }
            state.more = batch.more;
            state.older_loaded = true;
            notice = Some(match added {
                0 => "No older PRs".to_owned(),
                1 => "Loaded 1 older PR".to_owned(),
                n => format!("Loaded {n} older PRs"),
            });
            prs
        } else {
            let keep_older = state.older_loaded && group.is_paged();
            let mut prs = batch.prs;
            for old in existing {
                if prs.iter().any(|fresh| fresh.id == old.id) {
                    continue;
                }
                let in_group = PrGroup::of(&old.status) == group;
                if !in_group || keep_older || Some(old.id) == viewing {
                    prs.push(old);
                }
            }
            if !state.older_loaded {
                state.more = batch.more;
            }
            prs
        };
        state.loaded = true;
        // Providers give each group newest first; keep the groups in a fixed order.
        prs.sort_by_key(|pr| PrGroup::of(&pr.status));
        store.cache.prs = LoadState::Loaded(prs);
        if let Some(message) = notice {
            store.notice = Some(Notice::info(message));
        }
    }

    fn pr_data_mut(&mut self, pr_id: PrId) -> &mut PrData {
        self.state.store.cache.details.entry(pr_id).or_default()
    }
}
