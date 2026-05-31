#[cfg(test)]
mod tests {
    use crate::temporal::{
        after_start, before_end, must_be_future, not_future, resolution_after_onset,
        within_validity_window, CLOCK_SKEW_SECS, MAX_SCHEDULE_WINDOW_SECS,
        MAX_VALIDITY_WINDOW_SECS,
    };
    use soroban_sdk::testutils::Ledger;
    use soroban_sdk::Env;

    // Y2038 boundary: i32::MAX seconds since Unix epoch = 2038-01-19T03:14:07Z
    const Y2038_BOUNDARY: u64 = i32::MAX as u64; // 2_147_483_647

    fn env_at(ts: u64) -> Env {
        let env = Env::default();
        env.ledger().set_timestamp(ts);
        env
    }

    // ── Y2038 ────────────────────────────────────────────────────────────────

    #[test]
    fn y2038_timestamp_is_accepted_as_past() {
        // A ledger already past Y2038 should accept Y2038 as a past event.
        let env = env_at(Y2038_BOUNDARY + 1);
        assert!(not_future(&env, Y2038_BOUNDARY).is_ok());
    }

    #[test]
    fn y2038_timestamp_is_rejected_as_future_when_ledger_is_before() {
        // Ledger is one second before Y2038; Y2038 is in the future beyond skew.
        let env = env_at(Y2038_BOUNDARY - CLOCK_SKEW_SECS - 1);
        assert!(not_future(&env, Y2038_BOUNDARY).is_err());
    }

    #[test]
    fn just_above_y2038_is_valid_future() {
        let env = env_at(Y2038_BOUNDARY);
        assert!(must_be_future(&env, Y2038_BOUNDARY + 1).is_ok());
    }

    #[test]
    fn validity_window_spanning_y2038_boundary() {
        let start = Y2038_BOUNDARY - 100;
        let end = Y2038_BOUNDARY + 100;
        // Window is 200 s, well within MAX_VALIDITY_WINDOW_SECS.
        assert!(within_validity_window(start, end, MAX_VALIDITY_WINDOW_SECS).is_ok());
    }

    // ── UTC midnight boundaries ───────────────────────────────────────────────

    #[test]
    fn exactly_midnight_utc_accepted_as_past() {
        // 2024-01-01T00:00:00Z = 1_704_067_200
        let midnight: u64 = 1_704_067_200;
        let env = env_at(midnight);
        assert!(not_future(&env, midnight).is_ok());
    }

    #[test]
    fn one_second_before_midnight_is_past() {
        let midnight: u64 = 1_704_067_200;
        let env = env_at(midnight);
        assert!(not_future(&env, midnight - 1).is_ok());
    }

    #[test]
    fn one_second_after_midnight_is_future_when_ledger_is_at_midnight() {
        let midnight: u64 = 1_704_067_200;
        let env = env_at(midnight);
        assert!(must_be_future(&env, midnight + 1).is_ok());
    }

    #[test]
    fn window_crossing_midnight_is_valid() {
        let before_midnight: u64 = 1_704_067_200 - 30;
        let after_midnight: u64 = 1_704_067_200 + 30;
        assert!(
            within_validity_window(before_midnight, after_midnight, MAX_VALIDITY_WINDOW_SECS)
                .is_ok()
        );
    }

    // ── Leap-second window (23:59:60 UTC) ────────────────────────────────────
    // POSIX/Unix time does not represent leap seconds; 23:59:60 maps to the
    // same Unix timestamp as 00:00:00 of the next day.  The validator must
    // therefore treat such timestamps identically to the surrounding seconds.

    #[test]
    fn leap_second_window_treated_as_normal_timestamp() {
        // 2016-12-31T23:59:60Z (leap second) ≡ 2017-01-01T00:00:00Z = 1_483_228_800
        let leap_ts: u64 = 1_483_228_800;
        let env = env_at(leap_ts);
        assert!(not_future(&env, leap_ts).is_ok());
        assert!(not_future(&env, leap_ts - 1).is_ok());
    }

    #[test]
    fn window_around_leap_second_is_valid() {
        let leap_ts: u64 = 1_483_228_800;
        assert!(within_validity_window(leap_ts - 1, leap_ts + 1, MAX_VALIDITY_WINDOW_SECS).is_ok());
    }

    // ── Overflow ─────────────────────────────────────────────────────────────

    #[test]
    fn saturating_add_prevents_overflow_in_not_future() {
        // u64::MAX as ledger timestamp: saturating_add(CLOCK_SKEW_SECS) must
        // not panic or wrap.
        let env = env_at(u64::MAX);
        // Any ts ≤ u64::MAX is accepted (saturating_add clamps to u64::MAX).
        assert!(not_future(&env, u64::MAX).is_ok());
    }

    #[test]
    fn within_validity_window_rejects_overflow_duration() {
        // end - start would overflow if end < start; the guard catches it.
        assert!(within_validity_window(u64::MAX, 0, MAX_VALIDITY_WINDOW_SECS).is_err());
    }

    #[test]
    fn within_validity_window_rejects_max_u64_window() {
        // A window of u64::MAX seconds exceeds every allowed max.
        assert!(within_validity_window(0, u64::MAX, MAX_VALIDITY_WINDOW_SECS).is_err());
        assert!(within_validity_window(0, u64::MAX, MAX_SCHEDULE_WINDOW_SECS).is_err());
    }

    // ── Zero / epoch ─────────────────────────────────────────────────────────

    #[test]
    fn zero_timestamp_is_past_when_ledger_is_nonzero() {
        let env = env_at(1_000);
        assert!(not_future(&env, 0).is_ok());
    }

    #[test]
    fn zero_timestamp_is_not_future_when_ledger_is_zero() {
        let env = env_at(0);
        // ts == ledger: not_future allows it (within skew).
        assert!(not_future(&env, 0).is_ok());
    }

    #[test]
    fn zero_timestamp_fails_must_be_future_when_ledger_is_zero() {
        let env = env_at(0);
        // 0 is not strictly after 0.
        assert!(must_be_future(&env, 0).is_err());
    }

    #[test]
    fn zero_start_zero_end_window_is_invalid() {
        assert!(within_validity_window(0, 0, MAX_VALIDITY_WINDOW_SECS).is_err());
    }

    // ── after_start / before_end / resolution_after_onset ────────────────────

    #[test]
    fn after_start_rejects_equal_timestamps() {
        assert!(after_start(100, 100).is_err());
    }

    #[test]
    fn after_start_accepts_strictly_greater() {
        assert!(after_start(100, 101).is_ok());
    }

    #[test]
    fn before_end_rejects_equal_timestamps() {
        assert!(before_end(100, 100).is_err());
    }

    #[test]
    fn before_end_accepts_strictly_less() {
        assert!(before_end(99, 100).is_ok());
    }

    #[test]
    fn resolution_after_onset_at_y2038_boundary() {
        assert!(resolution_after_onset(Y2038_BOUNDARY, Y2038_BOUNDARY + 1).is_ok());
        assert!(resolution_after_onset(Y2038_BOUNDARY, Y2038_BOUNDARY).is_err());
    }

    // ── imaging-radiology timestamp arithmetic ────────────────────────────────
    // Mirrors the contract's use of temporal::must_be_future (schedule_imaging)
    // and temporal::not_future (upload_images) at boundary values.

    #[test]
    fn imaging_schedule_time_at_y2038_plus_one_is_valid_future() {
        let env = env_at(Y2038_BOUNDARY);
        assert!(must_be_future(&env, Y2038_BOUNDARY + 1).is_ok());
    }

    #[test]
    fn imaging_study_date_at_y2038_is_valid_past() {
        let env = env_at(Y2038_BOUNDARY + 1);
        assert!(not_future(&env, Y2038_BOUNDARY).is_ok());
    }

    #[test]
    fn imaging_study_date_zero_is_valid_past() {
        let env = env_at(1_000);
        assert!(not_future(&env, 0).is_ok());
    }
}
