use super::{
    Dimension, Edition,
    ores::{
        OreKind, OreScanRequest, scan_ores, scan_ores_with_budget_cancellable,
    },
    parse_seed, resolve_java_version,
};
use serde::Deserialize;
use std::{
    io,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

static ACTIVE_REQUEST: AtomicU64 = AtomicU64::new(0);
static CANCELED_REQUEST: AtomicU64 = AtomicU64::new(0);
static GENERATOR: Mutex<()> = Mutex::new(());
#[cfg(test)]
static CANCEL_CHECKS: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeRequest {
    pub id: u64,
    pub seed: String,
    pub edition: Edition,
    pub version: String,
    pub dimension: Dimension,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    #[serde(default)]
    pub chunk: bool,
}

pub fn begin(id: u64) {
    ACTIVE_REQUEST.store(id, Ordering::Relaxed);
}

pub fn cancel(id: u64) {
    CANCELED_REQUEST.store(id, Ordering::Relaxed);
    let _ = ACTIVE_REQUEST.compare_exchange(
        id,
        0,
        Ordering::Relaxed,
        Ordering::Relaxed,
    );
}

fn check_active(id: u64) -> io::Result<()> {
    if is_canceled(id) != 0 {
        return Err(io::Error::other("Underground generation canceled."));
    }
    Ok(())
}

extern "C" fn is_canceled(id: u64) -> i32 {
    #[cfg(test)]
    CANCEL_CHECKS.fetch_add(1, Ordering::Relaxed);
    i32::from(
        id == 0
            || ACTIVE_REQUEST.load(Ordering::Relaxed) != id
            || CANCELED_REQUEST.load(Ordering::Relaxed) == id,
    )
}

#[cfg(test)]
extern "C" fn never_canceled(_: u64) -> i32 {
    0
}

/// Legacy schema 1 returns a 64-cube volume. Chunk schema 3 returns full-height
/// padded columns and surface heights. Flags bit 0 means mineral results truncated.
pub fn generate_volume(request: VolumeRequest) -> io::Result<Vec<u8>> {
    check_active(request.id)?;
    let _guard = GENERATOR
        .lock()
        .map_err(|error| io::Error::other(error.to_string()))?;
    check_active(request.id)?;
    let (mc, code) = resolve_java_version(&request.version)?;
    let min_y =
        if matches!(request.dimension, Dimension::Overworld) && code >= 11800 {
            -64
        } else {
            0
        };
    let max_y = match request.dimension {
        Dimension::Overworld => {
            if code >= 11800 {
                320
            } else {
                256
            }
        }
        Dimension::Nether => 128,
        Dimension::End => {
            return Err(io::Error::other(
                "The End has no supported underground volume.",
            ));
        }
    };
    if code < 11400
        || request.x % 16 != 0
        || request.z % 16 != 0
        || request.x < -30_000_000
        || request.x
            > if request.chunk {
                29_999_984
            } else {
                29_999_936
            }
        || request.z < -30_000_000
        || request.z
            > if request.chunk {
                29_999_984
            } else {
                29_999_936
            }
        || request.y < min_y
        || if request.chunk {
            request.y != min_y
        } else {
            request.y > max_y - 64 || (request.y - min_y) % 8 != 0
        }
    {
        return Err(io::Error::other(
            "Invalid underground region. A 64 × 64 × 64 block region within Java 1.14+ world bounds is required.",
        ));
    }
    if request.chunk {
        return generate_chunk(&request, mc, code);
    }
    let mut data = vec![0_u8; 64 * 64 * 64];
    let mut truncated = false;
    let mut chunk = vec![0_u8; 16 * 16 * 64];
    let kinds = if matches!(request.dimension, Dimension::Nether) {
        vec![OreKind::Netherite, OreKind::Quartz, OreKind::NetherGold]
    } else {
        vec![
            OreKind::Diamond,
            OreKind::Iron,
            OreKind::IronVein,
            OreKind::Copper,
            OreKind::CopperVein,
            OreKind::Gold,
            OreKind::Redstone,
            OreKind::Lapis,
            OreKind::Coal,
            OreKind::Emerald,
        ]
    };
    for cx in 0..4 {
        for cz in 0..4 {
            check_active(request.id)?;
            let x = request.x + cx * 16;
            let z = request.z + cz * 16;
            let status = unsafe {
                super::axolotl_seed_map_underground_volume(
                    parse_seed(&request.seed),
                    mc,
                    request.edition.generator_flags(),
                    request.dimension.as_cubiomes(),
                    x,
                    z,
                    request.y,
                    16,
                    64,
                    chunk.as_mut_ptr(),
                    chunk.len() as u64,
                )
            };
            if status != 0 {
                return Err(io::Error::other(format!(
                    "Underground volume generation failed ({status})."
                )));
            }
            for bx in 0..16 {
                for bz in 0..16 {
                    for by in 0..64 {
                        let source = ((bx * 16 + bz) * 64 + by) as usize;
                        let target = (((cx * 16 + bx) * 64 + cz * 16 + bz) * 64
                            + by) as usize;
                        data[target] = match chunk[source] {
                            1 if matches!(
                                request.dimension,
                                Dimension::Nether
                            ) =>
                            {
                                3
                            }
                            1 if code >= 11800 && request.y + by < 0 => 2,
                            value => value,
                        };
                    }
                }
            }
        }
    }
    for cx in -1..5 {
        for cz in -1..5 {
            check_active(request.id)?;
            let source_x = request.x / 16 + cx;
            let source_z = request.z / 16 + cz;
            if source_x.abs_diff(0) > 1_875_000
                || source_z.abs_diff(0) > 1_875_000
            {
                continue;
            }
            let halo = !(0..4).contains(&cx) || !(0..4).contains(&cz);
            let scan_kinds = kinds
                .iter()
                .copied()
                .filter(|kind| {
                    !halo
                        || !matches!(
                            kind,
                            OreKind::IronVein | OreKind::CopperVein
                        )
                })
                .collect();
            let results = scan_ores(OreScanRequest {
                seed: request.seed.clone(),
                edition: request.edition,
                version: request.version.clone(),
                dimension: request.dimension,
                ores: scan_kinds,
                chunks: vec![source_x, source_z],
            })?;
            for result in results {
                truncated |= result.truncated;
                for hit in result.hits {
                    if hit.x < request.x
                        || hit.x >= request.x + 64
                        || hit.z < request.z
                        || hit.z >= request.z + 64
                        || hit.y < request.y
                        || hit.y >= request.y + 64
                    {
                        continue;
                    }
                    let index = (((hit.x - request.x) * 64 + hit.z - request.z)
                        * 64
                        + hit.y
                        - request.y) as usize;
                    let host = data[index];
                    if !(1..=3).contains(&host) {
                        continue;
                    }
                    data[index] =
                        16 + hit.ore as u8 + if host == 2 { 64 } else { 0 };
                }
            }
        }
    }
    check_active(request.id)?;
    let mut response = Vec::with_capacity(data.len() + 16);
    for value in [1_u32, u32::from(truncated), 64, 64] {
        response.extend_from_slice(&value.to_le_bytes());
    }
    response.extend(data);
    Ok(response)
}

/// Schema 3 contains an 18-wide X/Z volume across the complete build height,
/// then i16 surface Y coordinates. Only the inner 16 columns render.
fn generate_chunk(
    request: &VolumeRequest,
    mc: i32,
    code: i32,
) -> io::Result<Vec<u8>> {
    let min_y =
        if matches!(request.dimension, Dimension::Overworld) && code >= 11800 {
            -64
        } else {
            0
        };
    let max_y = match request.dimension {
        Dimension::Nether => 128,
        Dimension::Overworld if code >= 11800 => 320,
        _ => 256,
    };
    let height = max_y - min_y;
    let mut data = vec![0_u8; 18 * 18 * height as usize];
    let mut heights = vec![0_i32; 18 * 18];
    let status = unsafe {
        super::axolotl_seed_map_underground_chunk(
            parse_seed(&request.seed),
            mc,
            request.edition.generator_flags(),
            request.dimension.as_cubiomes(),
            request.x,
            request.z,
            data.as_mut_ptr(),
            data.len() as u64,
            heights.as_mut_ptr(),
            request.id,
            is_canceled,
        )
    };
    check_active(request.id)?;
    if status != 0 {
        return Err(io::Error::other(format!(
            "Underground chunk generation failed ({status})."
        )));
    }
    let surface: Vec<i16> = heights
        .into_iter()
        .map(|y| if y <= min_y { i16::MIN } else { (y - 1) as i16 })
        .collect();
    for (index, column) in data.chunks_exact_mut(height as usize).enumerate() {
        let x = request.x + (index / 18) as i32 - 1;
        let z = request.z + (index % 18) as i32 - 1;
        if !(-30_000_000..30_000_000).contains(&x)
            || !(-30_000_000..30_000_000).contains(&z)
        {
            continue;
        }
        for (by, value) in column.iter_mut().enumerate() {
            if *value == 1 {
                *value = if matches!(request.dimension, Dimension::Nether) {
                    3
                } else if code >= 11800 && min_y + (by as i32) < 0 {
                    2
                } else {
                    1
                };
            }
        }
    }
    let kinds = if matches!(request.dimension, Dimension::Nether) {
        vec![OreKind::Netherite, OreKind::Quartz, OreKind::NetherGold]
    } else {
        vec![
            OreKind::Diamond,
            OreKind::Iron,
            OreKind::IronVein,
            OreKind::Copper,
            OreKind::CopperVein,
            OreKind::Gold,
            OreKind::Redstone,
            OreKind::Lapis,
            OreKind::Coal,
            OreKind::Emerald,
        ]
    };
    let mut truncated = false;
    for cx in -1..=1 {
        for cz in -1..=1 {
            check_active(request.id)?;
            let cx_world = request.x / 16 + cx;
            let cz_world = request.z / 16 + cz;
            if cx_world.abs_diff(0) > 1_875_000
                || cz_world.abs_diff(0) > 1_875_000
            {
                continue;
            }
            let ores: Vec<_> = kinds
                .iter()
                .copied()
                .filter(|kind| {
                    (cx == 0 && cz == 0)
                        || !matches!(
                            kind,
                            OreKind::IronVein | OreKind::CopperVein
                        )
                })
                .collect();
            let capacity = 16_384 * ores.len();
            for result in scan_ores_with_budget_cancellable(
                OreScanRequest {
                    seed: request.seed.clone(),
                    edition: request.edition,
                    version: request.version.clone(),
                    dimension: request.dimension,
                    ores,
                    chunks: vec![cx_world, cz_world],
                },
                capacity,
                Some((request.id, is_canceled)),
            )? {
                truncated |= result.truncated;
                for hit in result.hits {
                    let x = hit.x - request.x + 1;
                    let z = hit.z - request.z + 1;
                    let y = hit.y - request.y;
                    if !(0..18).contains(&x)
                        || !(0..18).contains(&z)
                        || !(0..height).contains(&y)
                    {
                        continue;
                    }
                    let index = ((x * 18 + z) * height + y) as usize;
                    let host = data[index];
                    if (1..=3).contains(&host) {
                        data[index] =
                            16 + hit.ore as u8 + if host == 2 { 64 } else { 0 };
                    }
                }
            }
        }
    }
    check_active(request.id)?;
    let mut response = Vec::with_capacity(16 + data.len() + surface.len() * 2);
    for value in [3_u32, u32::from(truncated), 18, height as u32] {
        response.extend_from_slice(&value.to_le_bytes());
    }
    response.extend(data);
    for value in surface {
        response.extend_from_slice(&value.to_le_bytes());
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_bounds_cancellation_and_materials() {
        let request = VolumeRequest {
            id: 99,
            seed: "12345".into(),
            edition: Edition::Java,
            version: "26.3".into(),
            dimension: Dimension::Overworld,
            x: -32,
            y: -64,
            z: -32,
            chunk: false,
        };
        begin(request.id);
        let mut invalid = request.clone();
        invalid.y = i32::MAX;
        assert!(generate_volume(invalid).is_err());
        let bytes = generate_volume(request.clone()).unwrap();
        assert_eq!(bytes.len(), 16 + 64 * 64 * 64);
        assert_eq!(&bytes[..4], &1_u32.to_le_bytes());
        assert!(bytes[16..].contains(&0));
        assert!(bytes[16..].contains(&2));
        for (index, id) in bytes[16..].iter().enumerate() {
            let y = request.y + (index % 64) as i32;
            if *id == 5 {
                assert!(y < -54);
            }
            if *id == 4 {
                assert!(y < 63);
            }
            assert!(
                *id <= 6 || (16..=28).contains(id) || (80..=92).contains(id)
            );
        }
        let mut chunk = request.clone();
        chunk.chunk = true;
        let ore_request = || OreScanRequest {
            seed: request.seed.clone(),
            edition: request.edition,
            version: request.version.clone(),
            dimension: request.dimension,
            ores: vec![OreKind::Diamond, OreKind::Iron],
            chunks: vec![0, 0],
        };
        let small = super::super::ores::scan_ores_with_budget(ore_request(), 2)
            .unwrap();
        let expanded =
            super::super::ores::scan_ores_with_budget(ore_request(), 32_768)
                .unwrap();
        assert!(small[0].truncated);
        assert!(expanded[0].hits.len() > small[0].hits.len());
        assert!(!expanded[0].truncated);
        let cancellable = scan_ores_with_budget_cancellable(
            ore_request(),
            32_768,
            Some((99, never_canceled)),
        )
        .unwrap();
        assert_eq!(expanded[0].hits.len(), cancellable[0].hits.len());
        assert!(
            expanded[0]
                .hits
                .iter()
                .zip(&cancellable[0].hits)
                .all(|(a, b)| (a.ore, a.x, a.y, a.z) == (b.ore, b.x, b.y, b.z))
        );
        let bytes = generate_volume(chunk.clone()).unwrap();
        assert_eq!(bytes.len(), 16 + 18 * 18 * 384 + 18 * 18 * 2);
        assert_eq!(&bytes[..4], &3_u32.to_le_bytes());
        let heights = &bytes[16 + 18 * 18 * 384..];
        assert!(
            heights
                .chunks_exact(2)
                .any(|value| i16::from_le_bytes([value[0], value[1]]) >= 0)
        );
        chunk.x = -30_000_000;
        chunk.z = -30_000_000;
        assert!(generate_volume(chunk).is_ok());
        cancel(request.id);
        assert!(generate_volume(request).is_err());
        let interrupted = VolumeRequest {
            id: 100,
            seed: "12345".into(),
            edition: Edition::Java,
            version: "26.3".into(),
            dimension: Dimension::Overworld,
            x: -32,
            y: -64,
            z: -32,
            chunk: true,
        };
        CANCEL_CHECKS.store(0, Ordering::Relaxed);
        begin(interrupted.id);
        let task = std::thread::spawn(move || generate_volume(interrupted));
        let started = std::time::Instant::now();
        while CANCEL_CHECKS.load(Ordering::Relaxed) < 10 && !task.is_finished()
        {
            assert!(started.elapsed() < std::time::Duration::from_secs(2));
            std::thread::yield_now();
        }
        assert!(
            !task.is_finished(),
            "Generation must still be active when canceled"
        );
        let canceled_at = std::time::Instant::now();
        cancel(100);
        assert!(task.join().unwrap().is_err());
        eprintln!(
            "Active native generation canceled in {:?}",
            canceled_at.elapsed()
        );
        assert!(canceled_at.elapsed() < std::time::Duration::from_millis(250));
        let checks_after = CANCEL_CHECKS.load(Ordering::Relaxed);
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert_eq!(CANCEL_CHECKS.load(Ordering::Relaxed), checks_after);
        cancel(101);
        begin(101);
        assert!(
            generate_volume(VolumeRequest {
                id: 101,
                seed: "1".into(),
                edition: Edition::Java,
                version: "26.3".into(),
                dimension: Dimension::Overworld,
                x: 0,
                y: -64,
                z: 0,
                chunk: true
            })
            .is_err()
        );
    }
}
