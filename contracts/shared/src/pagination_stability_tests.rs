/// Cursor-stability tests for the shared pagination module.
///
/// # Cursor strategy
///
/// The pagination module uses a **page-index / offset-based** strategy:
/// - Items are appended to fixed-size pages (≤ `MAX_PAGE_SIZE` items each).
/// - A cursor is a `u32` page index.  Callers advance by passing the returned
///   `next_page` value back as the `page` argument.
///
/// ## Stability under insertion
///
/// Because items are only ever *appended* (never inserted in the middle), a
/// cursor pointing to page N always returns the same items that were on page N
/// when it was first read, **provided no items have been deleted**.  New items
/// land on the current head page or a new page beyond it, so earlier pages are
/// immutable once full.
///
/// ## Stability under deletion
///
/// The current implementation does not support deletion of individual items
/// from a page.  Contracts that need logical deletion should mark items as
/// inactive in their own storage rather than removing them from the page index.
/// This preserves cursor stability.
///
/// ## Documented limitation
///
/// If the head page is not yet full when a cursor is issued, a concurrent
/// insertion onto that same page will cause the next read of that page to
/// return more items than the first read.  This is expected behaviour for an
/// append-only log and does not cause records to be *skipped* or *duplicated*
/// across pages.
///
/// Closes #399.
#[cfg(test)]
mod pagination_stability_tests {
    use crate::pagination::{get_paged, push_paged, PageResult, MAX_PAGE_SIZE, NO_NEXT_PAGE};
    use soroban_sdk::{contracttype, Env};

    #[contracttype]
    #[derive(Clone)]
    enum TestKey {
        Page(u32),
        Head,
    }

    fn push(env: &Env, id: u64) {
        push_paged(env, |p| TestKey::Page(p), || TestKey::Head, id);
    }

    fn get(env: &Env, page: u32) -> PageResult {
        get_paged(env, |p| TestKey::Page(p), || TestKey::Head, || 0, page)
    }

    // ── basic round-trip ─────────────────────────────────────────────────────

    #[test]
    fn empty_list_returns_empty_first_page() {
        let env = Env::default();
        let result = get(&env, 0);
        assert_eq!(result.ids.len(), 0);
        assert_eq!(result.next_page, NO_NEXT_PAGE);
    }

    #[test]
    fn single_item_on_page_zero() {
        let env = Env::default();
        push(&env, 42);
        let result = get(&env, 0);
        assert_eq!(result.ids.len(), 1);
        assert_eq!(result.ids.get(0).unwrap(), 42);
        assert_eq!(result.next_page, NO_NEXT_PAGE);
    }

    // ── cursor stability: insertion after cursor issued ───────────────────────

    #[test]
    fn items_inserted_after_full_page_do_not_affect_earlier_page() {
        let env = Env::default();

        // Fill page 0 completely.
        for i in 0..MAX_PAGE_SIZE {
            push(&env, i as u64);
        }

        // Read page 0 — it is now full and immutable.
        let page0_before = get(&env, 0);
        assert_eq!(page0_before.ids.len() as u32, MAX_PAGE_SIZE);
        assert_eq!(page0_before.next_page, 1);

        // Insert more items (they land on page 1).
        for i in MAX_PAGE_SIZE..MAX_PAGE_SIZE + 5 {
            push(&env, i as u64);
        }

        // Re-read page 0 — must be identical.
        let page0_after = get(&env, 0);
        assert_eq!(page0_before.ids, page0_after.ids,
            "page 0 must be immutable after it was filled");
    }

    #[test]
    fn new_items_appear_on_subsequent_pages_not_earlier_ones() {
        let env = Env::default();

        // Fill page 0.
        for i in 0..MAX_PAGE_SIZE {
            push(&env, i as u64);
        }

        // Add one item to page 1.
        push(&env, 999);

        let page1 = get(&env, 1);
        assert_eq!(page1.ids.len(), 1);
        assert_eq!(page1.ids.get(0).unwrap(), 999);
        assert_eq!(page1.next_page, NO_NEXT_PAGE);
    }

    // ── no records skipped during concurrent insertion ────────────────────────

    #[test]
    fn full_scan_collects_all_items_despite_mid_scan_insertion() {
        let env = Env::default();

        // Insert enough items to fill two pages.
        let initial_count = MAX_PAGE_SIZE * 2;
        for i in 0..initial_count {
            push(&env, i as u64);
        }

        // Simulate a mid-scan insertion: read page 0, then insert, then read page 1.
        let page0 = get(&env, 0);
        assert_eq!(page0.ids.len() as u32, MAX_PAGE_SIZE);

        // Concurrent insertion lands on page 2 (pages 0 and 1 are full).
        push(&env, 9999);

        let page1 = get(&env, page0.next_page);
        assert_eq!(page1.ids.len() as u32, MAX_PAGE_SIZE);

        // The concurrent item is on page 2, not mixed into page 1.
        let page2 = get(&env, page1.next_page);
        assert_eq!(page2.ids.len(), 1);
        assert_eq!(page2.ids.get(0).unwrap(), 9999);

        // Collect all IDs into a fixed-size array (no_std compatible).
        // Max items = 2*MAX_PAGE_SIZE + 1 = 41; use 64 for safety.
        let mut all_ids = [0u64; 64];
        let mut count = 0usize;
        for id in page0.ids.iter() { all_ids[count] = id; count += 1; }
        for id in page1.ids.iter() { all_ids[count] = id; count += 1; }
        for id in page2.ids.iter() { all_ids[count] = id; count += 1; }

        // No duplicates.
        for i in 0..count {
            for j in (i + 1)..count {
                assert_ne!(all_ids[i], all_ids[j], "duplicate ID {} detected", all_ids[i]);
            }
        }

        // All original items present.
        for i in 0..initial_count {
            let target = i as u64;
            assert!(
                all_ids[..count].iter().any(|&x| x == target),
                "item {i} was skipped during pagination"
            );
        }
    }

    // ── no records duplicated when head page receives concurrent insertion ────

    #[test]
    fn partial_head_page_insertion_does_not_duplicate_items() {
        let env = Env::default();

        push(&env, 1);
        push(&env, 2);
        push(&env, 3);

        let first_read = get(&env, 0);
        assert_eq!(first_read.ids.len(), 3);

        // Concurrent insertion onto the same head page.
        push(&env, 4);

        let second_read = get(&env, 0);
        assert_eq!(second_read.ids.len(), 4);

        // Check no duplicates.
        let mut ids = [0u64; 4];
        for (i, id) in second_read.ids.iter().enumerate() { ids[i] = id; }
        for i in 0..4 {
            for j in (i + 1)..4 {
                assert_ne!(ids[i], ids[j], "duplicate ID on head page");
            }
        }
    }

    // ── next_page sentinel ────────────────────────────────────────────────────

    #[test]
    fn last_page_returns_no_next_page_sentinel() {
        let env = Env::default();
        push(&env, 1);
        let result = get(&env, 0);
        assert_eq!(result.next_page, NO_NEXT_PAGE);
    }

    #[test]
    fn full_page_returns_next_page_index() {
        let env = Env::default();
        for i in 0..MAX_PAGE_SIZE {
            push(&env, i as u64);
        }
        let result = get(&env, 0);
        assert_eq!(result.next_page, 1);
    }
}
