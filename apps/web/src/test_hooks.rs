#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;

#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_ASPECT_POPOVER_GEOMETRY_JSON: &str = "__qniAspectPopoverGeometryJson";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_APPLY_URL_PAYLOAD: &str = "__qniApplyUrlPayload";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_CIRCUIT_LIBRARY_DELETE: &str = "__qniCircuitLibraryDelete";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_CIRCUIT_LIBRARY_LOAD: &str = "__qniCircuitLibraryLoad";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_CIRCUIT_LIBRARY_RENAME: &str = "__qniCircuitLibraryRename";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_CIRCUIT_LIBRARY_SAVE: &str = "__qniCircuitLibrarySave";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_CIRCUIT_PICKER_DROPDOWN_GEOMETRY_JSON: &str =
    "__qniCircuitPickerDropdownGeometryJson";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_CIRCUIT_PICKER_GEOMETRY_JSON: &str = "__qniCircuitPickerGeometryJson";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_CIRCUIT_PICKER_RESIZE_GEOMETRY_JSON: &str =
    "__qniCircuitPickerResizeGeometryJson";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_CIRCUIT_PICKER_RENAME_GEOMETRY_JSON: &str =
    "__qniCircuitPickerRenameGeometryJson";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_CIRCUIT_PICKER_SNAPSHOT: &str = "__qniCircuitPickerSnapshot";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_GPU_PLAN_CAPACITY_ERROR: &str = "__qniGpuPlanCapacityError";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_HOVER_SNAPSHOT_JSON: &str = "__qniHoverSnapshotJson";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_ANGLE_INPUT_GEOMETRY_JSON: &str = "__qniAngleInputGeometryJson";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_LAYOUT_JSON: &str = "__qniLayoutJson";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_CIRCUIT_BLOCK_GEOMETRY_JSON: &str = "__qniCircuitBlockGeometryJson";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_SEED_CIRCUITS: &str = "__seedCircuits";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_SET_EXTERNAL_GPU_STATUS: &str = "__setExternalGpuStatus";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_TOOLBAR_DUPLICATE_GEOMETRY_JSON: &str = "__qniToolbarDuplicateGeometryJson";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_TOOLBAR_LOCK_GEOMETRY_JSON: &str = "__qniToolbarLockGeometryJson";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_TOOLBAR_RUN_GEOMETRY_JSON: &str = "__qniToolbarRunGeometryJson";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_TOOLBAR_GPU_STATUS_GEOMETRY_JSON: &str = "__qniToolbarGpuStatusGeometryJson";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_TOOLBAR_TUTORIAL_GEOMETRY_JSON: &str = "__qniToolbarTutorialGeometryJson";
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_TOOLBAR_TOOLTIP_TEXT: &str = "__qniToolbarTooltipText";

/// 起動完了フラグ。最初のフレームを描画した時点で立てる。
/// `bootstrap.ts` の `start()` 呼び出し直後に立てると、eframe が canvas の
/// イベントリスナを張る前にテストがクリックしてしまい入力が失われる。
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_EGUI_READY: &str = "__eguiReady";

/// 起動の進行段階。起動が固まったときに、どこで止まったかを切り分けるために publish する。
#[cfg(target_arch = "wasm32")]
pub(crate) const QNI_STARTUP_STAGE: &str = "__qniStartupStage";

#[cfg(target_arch = "wasm32")]
pub(crate) fn set_property(target: &JsValue, name: &str, value: &JsValue) {
    let _ = js_sys::Reflect::set(target, &JsValue::from_str(name), value);
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn set_window_value(name: &str, value: &JsValue) {
    let Some(window) = web_sys::window() else {
        return;
    };
    set_property(window.as_ref(), name, value);
}

/// 最初のフレーム描画後に一度だけ起動完了フラグを立てる。
#[cfg(target_arch = "wasm32")]
pub(crate) fn mark_egui_ready() {
    use std::sync::atomic::{AtomicBool, Ordering};

    static PUBLISHED: AtomicBool = AtomicBool::new(false);
    if PUBLISHED.swap(true, Ordering::Relaxed) {
        return;
    }
    set_window_value(QNI_EGUI_READY, &JsValue::TRUE);
    set_startup_stage("first-frame");
}

/// 起動の進行段階を publish する。値は `runner-start` / `app-new` / `first-frame`。
#[cfg(target_arch = "wasm32")]
pub(crate) fn set_startup_stage(stage: &str) {
    set_window_value(QNI_STARTUP_STAGE, &JsValue::from_str(stage));
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn mark_egui_ready() {}
