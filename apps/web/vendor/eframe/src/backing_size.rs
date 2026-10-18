pub(crate) fn choose_backing_size(
    css: (f64, f64),
    dpr: f64,
    device: Option<(f64, f64)>,
) -> (u32, u32) {
    let expected = ((css.0 * dpr).round(), (css.1 * dpr).round());
    // Preserve real-device pixel snapping, but reject inconsistent DPR emulation.
    let size = device
        .filter(|size| (size.0 - expected.0).abs() <= 1.0 && (size.1 - expected.1).abs() <= 1.0)
        .unwrap_or(expected);
    (size.0 as u32, size.1 as u32)
}

#[cfg(test)]
mod tests {
    use super::choose_backing_size;

    #[test]
    fn uses_agreeing_device_size() {
        assert_eq!(
            choose_backing_size((960.0, 640.0), 2.0, Some((1920.0, 1280.0))),
            (1920, 1280)
        );
    }

    #[test]
    fn rejects_emulated_dpr_mismatch() {
        assert_eq!(
            choose_backing_size((960.0, 640.0), 2.0, Some((960.0, 640.0))),
            (1920, 1280)
        );
    }

    #[test]
    fn preserves_fractional_dpr_device_snapping() {
        assert_eq!(
            choose_backing_size((100.4, 80.4), 1.25, Some((125.0, 100.0))),
            (125, 100)
        );
    }

    #[test]
    fn preserves_positive_device_snapping_offset() {
        assert_eq!(
            choose_backing_size((100.0, 80.0), 1.5, Some((151.0, 121.0))),
            (151, 121)
        );
    }

    #[test]
    fn rejects_two_pixel_device_offset() {
        assert_eq!(
            choose_backing_size((960.0, 640.0), 2.0, Some((1922.0, 1280.0))),
            (1920, 1280)
        );
    }

    #[test]
    fn falls_back_without_device_size() {
        assert_eq!(choose_backing_size((100.4, 80.4), 1.5, None), (151, 121));
    }

    #[test]
    fn rejects_device_size_when_only_height_disagrees() {
        assert_eq!(
            choose_backing_size((960.0, 640.0), 2.0, Some((1920.0, 640.0))),
            (1920, 1280)
        );
    }

    #[test]
    fn rejects_device_size_when_only_width_disagrees() {
        assert_eq!(
            choose_backing_size((960.0, 640.0), 2.0, Some((960.0, 1280.0))),
            (1920, 1280)
        );
    }
}
