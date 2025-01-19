use std::time::Instant;

pub struct MemWidgetState {
    pub current_display_time: u64,
    pub autohide_timer: Option<Instant>,

    // FIXME: (points_rework_v1) REMOVE THESE
    pub ram_points_cache: Vec<(f64, f64)>,
    pub swap_points_cache: Vec<(f64, f64)>,
    #[cfg(not(target_os = "windows"))]
    pub cache_points_cache: Vec<(f64, f64)>,
    #[cfg(feature = "zfs")]
    pub arc_points_cache: Vec<(f64, f64)>,
    #[cfg(feature = "gpu")]
    pub gpu_points_cache: Vec<Vec<(f64, f64)>>,
}

impl MemWidgetState {
    pub fn init(current_display_time: u64, autohide_timer: Option<Instant>) -> Self {
        MemWidgetState {
            current_display_time,
            autohide_timer,
            ram_points_cache: vec![],
            swap_points_cache: vec![],
            #[cfg(not(target_os = "windows"))]
            cache_points_cache: vec![],
            #[cfg(feature = "zfs")]
            arc_points_cache: vec![],
            gpu_points_cache: vec![],
        }
    }
}
