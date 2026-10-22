#[derive(Default)]
pub(super) struct GpuEnvironment {
    pub is_default: Option<bool>,
    pub name: String,
    pub environment: Vec<String>,
}

/// Only offload when the service identifies both the display GPU and an alternative.
/// Missing topology and NVIDIA-driven sessions keep their existing environment.
pub(super) fn select_offload_environment(
    gpus: Vec<GpuEnvironment>,
) -> Vec<(String, String)> {
    let defaults = gpus
        .iter()
        .filter(|gpu| gpu.is_default == Some(true))
        .collect::<Vec<_>>();
    if defaults.len() != 1 {
        return Vec::new();
    }
    let name = defaults[0].name.to_ascii_lowercase();
    if name.is_empty()
        || ["nvidia", "geforce", "quadro"]
            .iter()
            .any(|vendor| name.contains(vendor))
    {
        return Vec::new();
    }
    for gpu in gpus.into_iter().filter(|gpu| gpu.is_default == Some(false)) {
        if gpu.environment.is_empty() || gpu.environment.len() % 2 != 0 {
            continue;
        }
        if gpu.environment.chunks_exact(2).any(|entry| {
            entry[0].is_empty()
                || entry[0].contains('=')
                || entry.iter().any(|part| part.contains('\0'))
        }) {
            continue;
        }
        return gpu
            .environment
            .chunks_exact(2)
            .map(|entry| (entry[0].clone(), entry[1].clone()))
            .collect();
    }
    Vec::new()
}

pub(super) fn glx_renderer_is_nvidia(output: &str) -> Option<bool> {
    output.lines().find_map(|line| {
        let vendor = line.trim().strip_prefix("OpenGL vendor string:")?.trim();
        (!vendor.is_empty())
            .then(|| vendor.to_ascii_lowercase().contains("nvidia"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn gpu(
        name: &str,
        is_default: Option<bool>,
        environment: &[&str],
    ) -> GpuEnvironment {
        GpuEnvironment {
            name: name.into(),
            is_default,
            environment: environment
                .iter()
                .map(|value| value.to_string())
                .collect(),
        }
    }
    #[test]
    fn current_glx_renderer_takes_precedence_over_boot_gpu_topology() {
        assert_eq!(
            glx_renderer_is_nvidia("OpenGL vendor string: NVIDIA Corporation"),
            Some(true)
        );
        assert_eq!(
            glx_renderer_is_nvidia("OpenGL vendor string: Intel"),
            Some(false)
        );
        assert_eq!(glx_renderer_is_nvidia("unavailable"), None);
    }
    #[test]
    fn hybrid_uses_only_service_provided_environment() {
        let env = select_offload_environment(vec![
            gpu("Intel", Some(true), &[]),
            gpu("NVIDIA", Some(false), &["__NV_PRIME_RENDER_OFFLOAD", "1"]),
        ]);
        assert_eq!(env, vec![("__NV_PRIME_RENDER_OFFLOAD".into(), "1".into())]);
    }
    #[test]
    fn nvidia_display_and_single_gpu_do_not_offload() {
        assert!(
            select_offload_environment(vec![
                gpu("NVIDIA RTX", Some(true), &[]),
                gpu("Intel", Some(false), &["DRI_PRIME", "1"])
            ])
            .is_empty()
        );
        assert!(
            select_offload_environment(vec![gpu("AMD", Some(true), &[])])
                .is_empty()
        );
    }
    #[test]
    fn missing_or_malformed_topology_does_not_guess() {
        assert!(select_offload_environment(vec![]).is_empty());
        assert!(
            select_offload_environment(vec![gpu(
                "NVIDIA",
                Some(false),
                &["DRI_PRIME", "1"]
            )])
            .is_empty()
        );
        assert!(
            select_offload_environment(vec![
                gpu("Intel", Some(true), &[]),
                gpu("NVIDIA", Some(false), &["DRI_PRIME"])
            ])
            .is_empty()
        );
    }
}
