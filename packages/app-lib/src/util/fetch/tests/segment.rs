//! Fetch segment tests.

use crate::util::download::constants::{
    FILE_TRANSFER_FIRST_BYTE_TIMEOUT, FILE_VALIDATION_CONCURRENCY,
    MAX_SEGMENT_CONCURRENCY, MIN_SEGMENT_SIZE, REASSIGNABLE_FIRST_BYTE_TIMEOUT,
    SEGMENT_EXPANSION_INTERVAL, SEGMENT_EXPANSION_SAMPLE_COUNT,
    SEGMENTED_DOWNLOAD_THRESHOLD,
};
use crate::util::download_manager::SpeedSnapshot;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Semaphore;

use super::super::*;
use super::fixtures::*;

#[test]
fn verified_file_classes_share_range_growth_and_recovery() {
    for resource in [
        ResourceClass::MinecraftAsset,
        ResourceClass::MinecraftLibrary,
        ResourceClass::Loader,
        ResourceClass::Java,
        ResourceClass::Modrinth,
        ResourceClass::CurseForge,
        ResourceClass::Modpack,
        ResourceClass::Other,
    ] {
        let request =
            DownloadRequest::new("https://example.com/file", resource)
                .with_integrity(
                    Integrity::sha1("expected").with_size(16 * 1024 * 1024),
                );
        assert!(
            request.prefers_independent_ranges(16 * 1024 * 1024),
            "{resource:?}"
        );
        assert_eq!(
            requested_range_floor(
                &request,
                16 * 1024 * 1024,
                16 * 1024 * 1024,
                64
            ),
            32
        );
        assert_eq!(
            requested_range_floor(
                &request,
                16 * 1024 * 1024,
                MIN_SEGMENT_SIZE * 3,
                64
            ),
            3
        );
        assert_eq!(
            requested_range_floor(
                &request,
                16 * 1024 * 1024,
                16 * 1024 * 1024,
                2
            ),
            2
        );
        assert!(can_recover_file_ranges(&request, MIN_SEGMENT_SIZE * 2));
        assert!(
            !request
                .clone()
                .with_http1_segmented_download(false)
                .supports_range_recovery()
        );
        assert!(
            !request
                .clone()
                .with_segmented_download(false)
                .supports_range_recovery()
        );
        assert!(
            !request
                .with_integrity(Integrity::default())
                .supports_range_recovery()
        );
    }
    let metadata = DownloadRequest::new(
        "https://example.com/metadata",
        ResourceClass::Metadata,
    )
    .with_integrity(Integrity::sha1("expected").with_size(16 * 1024 * 1024));
    assert!(!metadata.supports_range_recovery());
}

#[test]
fn dynamic_ranges_split_the_largest_remaining_tail() {
    let range = DownloadRange::new(0, 0, 10 * 1024 * 1024 - 1);
    let tail = range.split_tail(1).unwrap();
    assert_eq!(range.end(), 6 * 1024 * 1024 - 1);
    assert_eq!(tail.start, 6 * 1024 * 1024);
    assert_eq!(tail.end(), 10 * 1024 * 1024 - 1);
    assert!(tail.remaining() >= 256 * 1024);

    let small = DownloadRange::new(2, 0, 256 * 1024 - 2);
    assert!(small.split_tail(3).is_none());
}

#[test]
fn large_files_start_parallel_ranges_without_consulting_speed_floor() {
    let size = 16 * 1024 * 1024;
    assert_eq!(initial_segment_count(size, 64), 16);
    let ranges = create_initial_ranges(size, 4);
    assert_eq!(ranges.len(), 4);
    assert_eq!(ranges.first().unwrap().start, 0);
    assert_eq!(ranges.last().unwrap().end(), size - 1);
    for pair in ranges.windows(2) {
        assert_eq!(pair[0].end() + 1, pair[1].start);
    }
}

#[test]
fn segmented_concurrency_respects_effective_and_global_limits() {
    assert_eq!(segmented_concurrency_cap(64), MAX_SEGMENT_CONCURRENCY);
    assert_eq!(segmented_concurrency_cap(8), 8);
    assert_eq!(segmented_concurrency_cap(4), 4);
    assert_eq!(segmented_concurrency_cap(1), 1);
    assert_eq!(initial_segment_count(16 * 1024 * 1024, 3), 3);
}

#[tokio::test]
async fn segmented_permits_wait_for_one_slot_instead_of_falling_back() {
    let semaphore = FetchSemaphore(Semaphore::new(FILE_VALIDATION_CONCURRENCY));
    let held = semaphore
        .0
        .acquire_many(FILE_VALIDATION_CONCURRENCY as u32)
        .await
        .unwrap();
    let route = direct_test_route(
        "https://example.com/file".to_string(),
        DownloadRouteSource::Official,
    );
    let started = Instant::now();
    let (_, permits) = tokio::join!(
        async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            drop(held);
        },
        acquire_initial_segment_permits(&route, &semaphore, 4,),
    );
    let permits = permits.unwrap();
    assert_eq!(permits.len(), 1);
    assert!(started.elapsed() >= Duration::from_millis(40));
}

#[test]
fn segmented_download_caps_are_provider_neutral() {
    let bmclapi = route(
        "https://bmclapi2.bangbang93.com/assets/file".to_string(),
        DownloadRouteSource::Bmclapi,
        true,
        true,
    );
    let official = route(
        "https://resources.download.minecraft.net/file".to_string(),
        DownloadRouteSource::Official,
        false,
        true,
    );

    let modrinth = route(
        "https://cdn.modrinth.com/data/project/version/file.jar".into(),
        DownloadRouteSource::Official,
        false,
        true,
    );
    let size = 8 * 1024 * 1024;
    assert_eq!(route_segmented_concurrency_cap(&bmclapi, 64, size), 32);
    assert_eq!(route_segmented_concurrency_cap(&official, 64, size), 32);
    assert_eq!(route_segmented_concurrency_cap(&modrinth, 64, size), 32);
    let bulk = 100_855_703;
    assert_eq!(route_segmented_concurrency_cap(&modrinth, 64, bulk), 56);
    assert_eq!(route_segmented_concurrency_cap(&modrinth, 96, bulk), 64);
}

#[test]
fn adaptive_ranges_can_expand_and_reserve_capacity_for_small_files() {
    let size = 128 * 1024 * 1024;
    let max = adaptive_range_cap(size, 8, 8, 0, false);
    assert_eq!(max, 8);
    assert!(initial_segment_count(size, 64) < 64);
    assert_eq!(adaptive_range_cap(8 * 1024 * 1024, 8, 8, 0, false), 2);
    assert_eq!(adaptive_range_cap(size, 8, 1, 0, false), 1);
    assert_eq!(adaptive_range_cap(size, 8, 8, 2, false), 2);
    assert_eq!(adaptive_range_cap(size, 8, 8, 4, false), 1);
    let official = route(
        "https://range-fairness.invalid/file".into(),
        DownloadRouteSource::Official,
        false,
        true,
    );
    assert_eq!(route_segmented_concurrency_cap(&official, 4, size), 2);
    assert_eq!(route_segmented_concurrency_cap(&official, 1, size), 1);
    assert_eq!(initial_segment_count(size, 64), 16);
}

#[test]
fn bulk_ranges_can_reach_high_concurrency_without_tiny_file_fanout() {
    let size = 100_855_703;
    assert_eq!(adaptive_range_cap(size, 64, 64, 0, false), 64);
    assert_eq!(adaptive_range_cap(size, 48, 64, 2, false), 2);
    assert_eq!(adaptive_range_cap(size, 48, 1, 0, false), 1);
    assert_eq!(adaptive_range_cap(8 * 1024 * 1024, 48, 64, 0, false), 2);
    assert_eq!(range_growth_target(4, 48, 4, false, 0, 0), 8);
    assert_eq!(range_growth_target(8, 48, 8, false, 0, 0), 16);
    assert_eq!(range_growth_target(16, 48, 16, false, 0, 0), 32);
    assert_eq!(range_growth_target(32, 48, 32, false, 0, 0), 48);
    assert_eq!(range_growth_target(2, 48, 16, true, 0, 0), 16);
    assert_eq!(range_growth_target(1, 64, 1, false, 10_000_000, 100_000), 3);
    assert_eq!(range_growth_target(1, 2, 1, false, 10_000_000, 100_000), 2);
    assert_eq!(range_growth_target(16, 64, 16, false, u64::MAX, 1), 64);
}

#[test]
fn tail_floor_scales_with_remaining_data_and_limits() {
    assert_eq!(
        range_connection_floor(100_000_000, 20_000_000, 64, false),
        16
    );
    assert_eq!(range_connection_floor(100_000_000, 20_000_000, 2, false), 2);
    assert_eq!(
        range_connection_floor(100_000_000, MIN_SEGMENT_SIZE * 3, 64, false),
        3
    );
    assert_eq!(range_connection_floor(100_000_000, 1, 64, false), 1);
    assert_eq!(range_connection_floor(9_999_999, 8_000_000, 64, false), 1);
    assert_eq!(range_connection_floor(6_000_000, 4_000_000, 64, true), 15);
    assert_eq!(adaptive_range_cap(6_000_000, 64, 64, 0, true), 22);
    assert_eq!(adaptive_range_cap(6_000_000, 64, 1, 0, true), 1);
    assert_eq!(adaptive_range_cap(6_000_000, 64, 64, 2, true), 2);
}

#[test]
fn tail_floor_recovery_requires_verified_and_splittable_file_data() {
    let mut request = DownloadRequest::new(
        "https://cdn.modrinth.com/tail.jar",
        ResourceClass::Modpack,
    );
    request.integrity.size = Some(6_000_000);
    request.integrity.sha1 = Some("verified-at-completion".into());
    assert!(can_recover_file_ranges(&request, 6_000_000));
    let mut request = request.with_batch_download_policy();
    assert!(can_recover_file_ranges(&request, 6_000_000));
    assert!(can_recover_file_ranges(&request, MIN_SEGMENT_SIZE * 2));
    assert!(!can_recover_file_ranges(&request, MIN_SEGMENT_SIZE * 2 - 1));
    request.integrity.sha1 = None;
    assert!(!can_recover_file_ranges(&request, 6_000_000));
}

#[test]
fn range_recovery_does_not_treat_completed_ranges_as_failed_growth() {
    let baseline = RangeExpansionBaseline {
        speed: 2_712_603,
        completed_ranges: 5,
        previous_target: 3,
    };
    assert_eq!(baseline.gained_throughput(589_316, 8), None);
    assert_eq!(baseline.gained_throughput(589_316, 5), Some(false));
    assert_eq!(baseline.gained_throughput(4_000_000, 5), Some(true));
}

#[test]
fn stalled_growth_can_retry_after_the_startup_burst_collapses() {
    assert!(range_growth_can_retry(5 * 1024 * 1024, 500 * 1024));
    assert!(!range_growth_can_retry(500 * 1024, 490 * 1024));
    assert!(!range_growth_can_retry(0, 0));
    assert!(!range_growth_can_retry(u64::MAX, u64::MAX));
    assert!(range_growth_can_retry(u64::MAX, 1024));
}

#[test]
fn stable_aggregate_throughput_allows_gradual_expansion() {
    let snapshot = SpeedSnapshot {
        aggregate_speed: 2 * 1024 * 1024,
        recent_average: 1900 * 1024,
        speed_floor: 1024 * 1024,
        sample_count: SEGMENT_EXPANSION_SAMPLE_COUNT,
    };
    assert_eq!(
        expansion_block_reason(
            snapshot,
            4,
            8,
            4,
            16 * 1024 * 1024,
            SEGMENT_EXPANSION_INTERVAL,
            SEGMENT_EXPANSION_INTERVAL,
        ),
        None
    );
    assert_eq!(
        expansion_block_reason(
            snapshot,
            4,
            8,
            0,
            16 * 1024 * 1024,
            SEGMENT_EXPANSION_INTERVAL,
            SEGMENT_EXPANSION_INTERVAL,
        ),
        Some("no global permit available")
    );
}

#[test]
fn low_aggregate_throughput_still_allows_more_ranges() {
    let snapshot = SpeedSnapshot {
        aggregate_speed: 64 * 1024,
        recent_average: 80 * 1024,
        speed_floor: 1024 * 1024,
        sample_count: SEGMENT_EXPANSION_SAMPLE_COUNT,
    };
    assert_eq!(
        expansion_block_reason(
            snapshot,
            1,
            8,
            7,
            16 * 1024 * 1024,
            SEGMENT_EXPANSION_INTERVAL,
            SEGMENT_EXPANSION_INTERVAL,
        ),
        None
    );
}

#[test]
fn low_throughput_only_switches_routes_before_fallback_rounds() {
    assert!(allow_low_throughput_route_switch(true, false));
    assert!(!allow_low_throughput_route_switch(true, true));
    assert!(!allow_low_throughput_route_switch(false, false));
}

#[test]
fn slow_http2_downgrades_only_on_the_first_attempt_round() {
    assert!(should_downgrade_slow_http2(reqwest::Version::HTTP_2, false));
    assert!(!should_downgrade_slow_http2(reqwest::Version::HTTP_2, true));
    assert!(!should_downgrade_slow_http2(
        reqwest::Version::HTTP_11,
        false
    ));
}

#[test]
fn reassignable_mirror_uses_shorter_first_byte_timeout() {
    let mirror = route(
        "https://mirror.example/file".to_string(),
        DownloadRouteSource::Bmclapi,
        true,
        true,
    );
    assert_eq!(
        native_first_byte_timeout(&mirror, true),
        REASSIGNABLE_FIRST_BYTE_TIMEOUT
    );
    assert_eq!(
        native_first_byte_timeout(&mirror, false),
        FILE_TRANSFER_FIRST_BYTE_TIMEOUT
    );
}

#[test]
fn official_fallback_excludes_alternate_sources() {
    let alternate = route(
        "https://alternate.example/file".to_string(),
        DownloadRouteSource::Alternate,
        false,
        true,
    );
    let official = route(
        "https://official.example/file".to_string(),
        DownloadRouteSource::Official,
        false,
        true,
    );
    assert_eq!(
        official_fallback_route(&[alternate, official.clone()]),
        Some(official)
    );
}

#[test]
fn segmented_download_starts_at_four_mebibytes() {
    assert!(!should_use_segmented_download(
        SEGMENTED_DOWNLOAD_THRESHOLD - 1,
        0,
    ));
    assert!(should_use_segmented_download(
        SEGMENTED_DOWNLOAD_THRESHOLD,
        0,
    ));
    assert!(should_use_segmented_download(
        24 * 1024 * 1024,
        16 * 1024 * 1024
    ));
    assert!(!should_use_segmented_download(
        24 * 1024 * 1024,
        23 * 1024 * 1024
    ));
}

fn create_initial_ranges(size: u64, count: usize) -> Vec<DownloadRange> {
    create_remaining_ranges(size, 0, count)
}
