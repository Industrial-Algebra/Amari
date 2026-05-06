pub fn direct_gpu_runtime_available() -> bool {
    if std::env::var_os("AMARI_GPU_FORCE_CPU").is_some() {
        return false;
    }

    if std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok() {
        return false;
    }

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
    {
        let display_ok = std::env::var("DISPLAY")
            .ok()
            .map(|value| !value.is_empty())
            .unwrap_or(false);
        let xdg_ok = std::env::var("XDG_RUNTIME_DIR")
            .ok()
            .map(|value| !value.is_empty())
            .unwrap_or(false);
        return display_ok || xdg_ok;
    }

    #[allow(unreachable_code)]
    true
}
