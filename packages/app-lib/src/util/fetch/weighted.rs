//! Remaining-byte shares for explicitly forced independent downloads.

use crate::util::download::constants::{
    FORCED_DOWNLOAD_CONCURRENCY_MAX, FORCED_MIN_SEGMENT_SIZE,
};
use parking_lot::Mutex;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, LazyLock};

static POOLS: LazyLock<Mutex<HashMap<usize, Pool>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Default)]
struct Pool {
    next_id: u64,
    entries: BTreeMap<u64, Entry>,
}

struct Entry {
    remaining: Option<u64>,
    limit: usize,
    active: usize,
}

#[derive(Debug)]
pub(crate) struct ForcedDownload {
    pool: usize,
    id: u64,
}

impl ForcedDownload {
    pub(crate) fn register(pool: usize, remaining: Option<u64>) -> Arc<Self> {
        // sched-order: theseus::util::fetch::weighted::POOLS temporary
        let mut pools = POOLS.lock();
        let state = pools.entry(pool).or_default();
        let id = state.next_id;
        state.next_id += 1;
        state.entries.insert(
            id,
            Entry {
                remaining,
                limit: FORCED_DOWNLOAD_CONCURRENCY_MAX,
                active: 0,
            },
        );
        Arc::new(Self { pool, id })
    }

    pub(crate) fn set_remaining(&self, remaining: u64) {
        // sched-order: theseus::util::fetch::weighted::POOLS temporary
        let mut pools = POOLS.lock();
        if let Some(entry) = pools
            .get_mut(&self.pool)
            .and_then(|pool| pool.entries.get_mut(&self.id))
        {
            entry.remaining = Some(remaining);
        }
    }

    pub(crate) fn quota(&self, total_limit: usize) -> usize {
        // sched-order: theseus::util::fetch::weighted::POOLS temporary
        let mut pools = POOLS.lock();
        let pool = pools.get_mut(&self.pool).expect("registered download pool");
        pool.entries
            .get_mut(&self.id)
            .expect("registered download")
            .limit = total_limit.min(FORCED_DOWNLOAD_CONCURRENCY_MAX);
        quota(pool, self.id)
    }

    /// Track a connection only after its actual resource permits are acquired.
    pub(crate) fn connection(self: &Arc<Self>) -> ForcedConnection {
        // sched-order: theseus::util::fetch::weighted::POOLS temporary
        let mut pools = POOLS.lock();
        pools
            .get_mut(&self.pool)
            .and_then(|pool| pool.entries.get_mut(&self.id))
            .expect("registered download")
            .active += 1;
        ForcedConnection {
            download: Arc::clone(self),
            released: false,
        }
    }
}

impl Drop for ForcedDownload {
    fn drop(&mut self) {
        // sched-order: theseus::util::fetch::weighted::POOLS temporary
        let mut pools = POOLS.lock();
        if let Some(pool) = pools.get_mut(&self.pool) {
            pool.entries.remove(&self.id);
            if pool.entries.is_empty() {
                pools.remove(&self.pool);
            }
        }
    }
}

pub(crate) struct ForcedConnection {
    download: Arc<ForcedDownload>,
    released: bool,
}

impl ForcedConnection {
    /// Once selected to yield, keep yielding without decrementing twice.
    /// The caller must flush its writer and release the physical permits.
    pub(crate) fn should_yield(&mut self, total_limit: usize) -> bool {
        if self.released {
            return true;
        }
        // sched-order: theseus::util::fetch::weighted::POOLS temporary
        let mut pools = POOLS.lock();
        let pool = pools
            .get_mut(&self.download.pool)
            .expect("registered download pool");
        pool.entries
            .get_mut(&self.download.id)
            .expect("registered download")
            .limit = total_limit.min(FORCED_DOWNLOAD_CONCURRENCY_MAX);
        let target = quota(pool, self.download.id);
        let entry = pool
            .entries
            .get_mut(&self.download.id)
            .expect("registered download");
        if entry.active > target {
            entry.active -= 1;
            self.released = true;
        }
        self.released
    }
}

impl Drop for ForcedConnection {
    fn drop(&mut self) {
        if !self.released {
            // sched-order: theseus::util::fetch::weighted::POOLS temporary
            let mut pools = POOLS.lock();
            pools
                .get_mut(&self.download.pool)
                .and_then(|pool| pool.entries.get_mut(&self.download.id))
                .expect("registered download")
                .active -= 1;
        }
    }
}

fn quota(pool: &Pool, id: u64) -> usize {
    let limit = pool
        .entries
        .values()
        .filter(|entry| entry.remaining != Some(0))
        .map(|entry| entry.limit)
        .min()
        .unwrap_or(0);
    let entries = pool
        .entries
        .iter()
        .filter(|(_, entry)| entry.remaining != Some(0))
        .take(limit)
        .map(|(&id, entry)| {
            let weight = u128::from(entry.remaining.unwrap_or(1));
            let cap = entry.remaining.map_or(1, |remaining| {
                usize::try_from(remaining / FORCED_MIN_SEGMENT_SIZE)
                    .unwrap_or(usize::MAX)
                    .clamp(1, limit)
            });
            (id, weight, cap)
        })
        .collect::<Vec<_>>();
    let Some(index) = entries.iter().position(|entry| entry.0 == id) else {
        return 0;
    };
    apportion(&entries, limit)[index]
}

/// Find bounded proportional shares before applying Hamilton rounding. Starting
/// with one reserved slot and weighting only the rest would change the ratio.
fn apportion(entries: &[(u64, u128, usize)], limit: usize) -> Vec<usize> {
    let total = limit.min(entries.iter().map(|entry| entry.2).sum());
    let mut shares = vec![1; entries.len()];
    if total == entries.len() {
        return shares;
    }
    // Each share is clamp(lambda * weight, 1, cap). Sweep the exact rational
    // breakpoints to find lambda without floating point or u64 multiplication.
    let mut events = Vec::new();
    for (index, &(_, weight, cap)) in entries.iter().enumerate() {
        if cap > 1 {
            events.push((1_u128, weight, index, false));
            events.push((cap as u128, weight, index, true));
        }
    }
    events.sort_by(|a, b| {
        (a.0 * b.1)
            .cmp(&(b.0 * a.1))
            .then_with(|| a.3.cmp(&b.3))
            .then_with(|| a.2.cmp(&b.2))
    });
    let mut free = vec![false; entries.len()];
    let mut fixed = entries.len();
    let mut weight_sum = 0_u128;
    for (bound, weight, index, upper) in events {
        if weight_sum > 0
            && fixed <= total
            && (total - fixed) as u128 * weight <= bound * weight_sum
        {
            let available = (total - fixed) as u128;
            let mut remainders = Vec::new();
            for (index, &(_, weight, _)) in entries.iter().enumerate() {
                if free[index] {
                    let numerator = available * weight;
                    shares[index] = (numerator / weight_sum) as usize;
                    remainders.push((numerator % weight_sum, index));
                }
            }
            remainders
                .sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
            let left = total - shares.iter().sum::<usize>();
            for &(_, index) in remainders.iter().take(left) {
                shares[index] += 1;
            }
            return shares;
        }
        if upper {
            free[index] = false;
            weight_sum -= weight;
            fixed += entries[index].2;
            shares[index] = entries[index].2;
        } else {
            free[index] = true;
            weight_sum += weight;
            fixed -= 1;
        }
    }
    shares
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn pool() -> usize {
        static NEXT_POOL: AtomicUsize = AtomicUsize::new(1);
        NEXT_POOL.fetch_add(1, Ordering::Relaxed)
    }

    fn register(pool: usize, sizes: &[u64]) -> Vec<Arc<ForcedDownload>> {
        sizes
            .iter()
            .map(|&size| ForcedDownload::register(pool, Some(size)))
            .collect()
    }

    fn quotas(downloads: &[Arc<ForcedDownload>], limit: usize) -> Vec<usize> {
        for download in downloads {
            download.quota(limit);
        }
        downloads
            .iter()
            .map(|download| download.quota(limit))
            .collect()
    }

    fn active(download: &ForcedDownload) -> usize {
        // sched-order: theseus::util::fetch::weighted::POOLS temporary
        POOLS.lock()[&download.pool].entries[&download.id].active
    }

    #[test]
    fn full_weight_hamilton_shares_and_total_ceiling() {
        let downloads = register(pool(), &[32 << 20, 16 << 20, 8 << 20]);
        assert_eq!(quotas(&downloads, 64), vec![37, 18, 9]);
        assert_eq!(quotas(&downloads, 72), vec![41, 21, 10]);
        assert_eq!(quotas(&downloads, 128), vec![73, 37, 18]);
        assert_eq!(quotas(&downloads, usize::MAX), vec![146, 73, 37]);
    }

    #[test]
    fn minimum_one_and_data_caps_redistribute_unused_slots() {
        let downloads = register(pool(), &[1, 16 << 10, 32 << 10, 1 << 20]);
        assert_eq!(quotas(&downloads, 10), vec![1, 1, 1, 7]);
        assert_eq!(quotas(&downloads, 72), vec![1, 1, 2, 64]);
        let tiny = register(pool(), &[1, 2, 3]);
        assert_eq!(quotas(&tiny, 72), vec![1, 1, 1]);
    }

    #[test]
    fn changes_completion_and_cancellation_reassign_shares() {
        let mut downloads = register(pool(), &[32 << 20, 16 << 20, 8 << 20]);
        assert_eq!(quotas(&downloads, 64), vec![37, 18, 9]);
        downloads[0].set_remaining(8 << 20);
        assert_eq!(quotas(&downloads, 64), vec![16, 32, 16]);
        downloads[1].set_remaining(0);
        assert_eq!(quotas(&downloads, 64), vec![32, 0, 32]);
        drop(downloads.remove(2));
        assert_eq!(quotas(&downloads, 64), vec![64, 0]);
    }

    #[test]
    fn only_excess_connections_yield_once_and_drop_releases_the_rest() {
        let download = ForcedDownload::register(pool(), Some(1 << 20));
        let mut connections =
            (0..4).map(|_| download.connection()).collect::<Vec<_>>();
        assert_eq!(active(&download), 4);
        assert!(connections[0].should_yield(2));
        assert!(connections[0].should_yield(2));
        assert!(connections[1].should_yield(2));
        assert!(!connections[2].should_yield(2));
        assert!(!connections[3].should_yield(2));
        assert_eq!(active(&download), 2);
        drop(connections);
        assert_eq!(active(&download), 0);
        let connection = download.connection();
        let group = download.pool;
        drop(download);
        drop(connection);
        // sched-order: theseus::util::fetch::weighted::POOLS temporary
        assert!(!POOLS.lock().contains_key(&group));
    }

    #[test]
    fn joining_download_reclaims_excess_and_completion_yields_every_connection()
    {
        let group = pool();
        let first = ForcedDownload::register(group, Some(1 << 20));
        assert_eq!(first.quota(4), 4);
        let mut connections =
            (0..4).map(|_| first.connection()).collect::<Vec<_>>();
        let second = ForcedDownload::register(group, Some(1 << 20));
        assert_eq!(second.quota(4), 2);
        assert_eq!(first.quota(4), 2);
        assert_eq!(
            connections
                .iter_mut()
                .map(|connection| usize::from(connection.should_yield(4)))
                .sum::<usize>(),
            2,
        );
        assert_eq!(active(&first), 2);
        first.set_remaining(0);
        assert!(
            connections
                .iter_mut()
                .all(|connection| connection.should_yield(4))
        );
        assert_eq!(active(&first), 0);
        assert_eq!(second.quota(4), 4);
        drop(connections);
        assert_eq!(active(&first), 0);
    }

    #[test]
    fn queued_downloads_enter_in_registration_order() {
        let downloads = register(pool(), &[1 << 20; 4]);
        assert_eq!(quotas(&downloads, 2), vec![1, 1, 0, 0]);
        downloads[0].set_remaining(0);
        assert_eq!(quotas(&downloads, 2), vec![0, 1, 1, 0]);
        assert_eq!(quotas(&downloads, 0), vec![0, 0, 0, 0]);
    }

    #[test]
    fn unknown_sizes_use_one_slot_until_discovered() {
        let group = pool();
        let unknown = ForcedDownload::register(group, None);
        let known = ForcedDownload::register(group, Some(1 << 20));
        let downloads = [unknown, known];
        assert_eq!(quotas(&downloads, 8), vec![1, 7]);
        downloads[0].set_remaining(1 << 20);
        assert_eq!(quotas(&downloads, 8), vec![4, 4]);
    }

    #[test]
    fn largest_byte_counts_remain_exact_and_pools_are_independent() {
        let downloads = register(pool(), &[u64::MAX, u64::MAX, u64::MAX]);
        assert_eq!(quotas(&downloads, 64), vec![22, 21, 21]);
        let other = register(pool(), &[u64::MAX]);
        assert_eq!(quotas(&other, 72), vec![72]);
        assert_eq!(quotas(&downloads, 64), vec![22, 21, 21]);
    }
}
