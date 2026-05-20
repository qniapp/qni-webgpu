use super::{
    CircuitEntry, CircuitKind, CircuitLibrary, CircuitOrigin, EMPTY_CIRCUIT_JSON,
    GROVER_SEARCH_JSON,
};

fn user(id: &str, name: &str, json: &str, locked: bool) -> CircuitEntry {
    CircuitEntry::user(id.to_owned(), name.to_owned(), json.to_owned(), 0, locked)
}

#[test]
fn seed_contains_quirk_named_samples() {
    let library = CircuitLibrary::seed();

    assert_eq!(
        (
            library.entries.len(),
            library.active_id.as_str(),
            library.entries[0].name.as_str(),
            library.entries[1].name.as_str(),
            library.entries[2].name.as_str(),
            library.entries[3].name.as_str(),
            library.entries.iter().all(CircuitEntry::locked),
        ),
        (
            4,
            "bell",
            "Bell state",
            "GHZ state",
            "QFT 4-qubit",
            "Grover Search",
            true,
        )
    );
}

#[test]
fn grover_seed_uses_expanded_quirk_json() {
    let library = CircuitLibrary::seed();
    let grover = library
        .entries
        .iter()
        .find(|entry| entry.id == "grover-search");

    assert_eq!(
        grover.map(|entry| entry.circuit_json.as_str()),
        Some(GROVER_SEARCH_JSON)
    );
}

#[test]
fn current_and_new_circuits_use_incrementing_default_names() {
    let mut library = CircuitLibrary::seed();

    library.set_active_current_circuit(EMPTY_CIRCUIT_JSON.to_owned());
    let initial_name = library.active().name.clone();

    library.set_active_current_circuit(r#"{"cols":[["H"]]}"#.to_owned());
    let edited_name = library.active().name.clone();

    let first = library.create_new().clone();
    let second = library.create_new().clone();

    assert_eq!(
        (
            initial_name.as_str(),
            edited_name.as_str(),
            first.name.as_str(),
            second.name.as_str()
        ),
        ("Circuit 1", "Circuit 1", "Circuit 2", "Circuit 3")
    );
}

#[test]
fn legacy_auto_untitled_entries_migrate_to_numbered_circuits() {
    let mut library = CircuitLibrary::from_entries(
        vec![
            user("current", "Untitled", EMPTY_CIRCUIT_JSON, false),
            user("circuit-8", "Untitled", EMPTY_CIRCUIT_JSON, false),
            user("ckt_saved", "Untitled", EMPTY_CIRCUIT_JSON, false),
        ],
        "current".to_owned(),
    );

    let migrated = library.migrate_legacy_default_names();
    assert_eq!(
        (
            migrated,
            library.entries[0].name.as_str(),
            library.entries[1].name.as_str(),
            library.entries[2].name.as_str(),
        ),
        (true, "Circuit 1", "Circuit 2", "Untitled")
    );
}

#[test]
fn update_and_set_active_keep_canonical_json_for_unlocked_user() {
    let mut library = CircuitLibrary::from_entries(
        vec![user("ghz", "GHZ state", r#"{"cols":[["X"]]}"#, false)],
        "ghz".to_owned(),
    );

    library.set_active("ghz".to_owned());
    library.update_active(EMPTY_CIRCUIT_JSON.to_owned());

    assert_eq!(
        (
            library.active().id.as_str(),
            library.active().circuit_json.as_str()
        ),
        ("ghz", EMPTY_CIRCUIT_JSON)
    );
}

#[test]
fn locked_active_update_is_ignored() {
    let mut library = CircuitLibrary::from_entries(
        vec![user("locked", "Locked", r#"{"cols":[["X"]]}"#, true)],
        "locked".to_owned(),
    );

    library.update_active(EMPTY_CIRCUIT_JSON.to_owned());

    assert_eq!(
        library.active().circuit_json.as_str(),
        r#"{"cols":[["X"]]}"#
    );
}

#[test]
fn duplicate_sample_creates_unlocked_user() {
    let mut library = CircuitLibrary::seed();

    let id = library.duplicate_active();

    assert_eq!(
        library
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .map(CircuitEntry::locked),
        Some(false)
    );
}

#[test]
fn duplicate_locked_user_creates_unlocked_user() {
    let mut library = CircuitLibrary::from_entries(
        vec![user("locked", "Locked", EMPTY_CIRCUIT_JSON, true)],
        "locked".to_owned(),
    );

    let id = library.duplicate_active();

    assert_eq!(
        library
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .map(CircuitEntry::locked),
        Some(false)
    );
}

#[test]
fn duplicate_unlocked_user_creates_unlocked_user() {
    let mut library = CircuitLibrary::from_entries(
        vec![user("open", "Open", EMPTY_CIRCUIT_JSON, false)],
        "open".to_owned(),
    );

    let id = library.duplicate_active();

    assert_eq!(
        library
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .map(CircuitEntry::locked),
        Some(false)
    );
}

#[test]
fn duplicate_active_inserts_into_my_section_and_numbers_copy_names() {
    let mut library = CircuitLibrary::seed();
    library.set_active("bell".to_owned());

    let first_id = library.duplicate_active();
    let second_id = library.duplicate_active();
    let third_id = library.duplicate_active();

    assert_eq!(
        (
            library.entries[4].id.as_str(),
            library.entries[4].name.as_str(),
            library.entries[5].id.as_str(),
            library.entries[5].name.as_str(),
            library.entries[6].id.as_str(),
            library.entries[6].name.as_str(),
            library.active_id.as_str(),
        ),
        (
            first_id.as_str(),
            "Bell state (copy)",
            second_id.as_str(),
            "Bell state (copy 2)",
            third_id.as_str(),
            "Bell state (copy 3)",
            third_id.as_str(),
        )
    );
}

#[test]
fn duplicate_active_skips_existing_copy_name_collisions() {
    let mut library = CircuitLibrary::from_entries(
        vec![
            user("bell", "Bell state", EMPTY_CIRCUIT_JSON, false),
            user("copy-1", "Bell state (copy)", EMPTY_CIRCUIT_JSON, false),
            user("copy-2", "Bell state (copy 2)", EMPTY_CIRCUIT_JSON, false),
        ],
        "bell".to_owned(),
    );

    let id = library.duplicate_active();

    assert_eq!(
        (library.active_id.as_str(), library.active().name.as_str()),
        (id.as_str(), "Bell state (copy 3)")
    );
}

#[test]
fn move_to_slot_reorders_only_user_entries() {
    let mut library = CircuitLibrary::seed();
    library.create_new();
    let second_id = library.create_new().id.clone();

    library.move_to_slot(5, 4);

    assert_eq!(
        (
            library.entries[0].id.as_str(),
            library.entries[1].id.as_str(),
            library.entries[2].id.as_str(),
            library.entries[3].id.as_str(),
            library.entries[4].id.as_str(),
        ),
        ("bell", "ghz", "qft-4", "grover-search", second_id.as_str())
    );
}

#[test]
fn move_to_slot_reorders_only_sample_entries() {
    let mut library = CircuitLibrary::seed();
    library.create_new();

    library.move_to_slot(0, 2);

    assert_eq!(
        (
            library.entries[0].id.as_str(),
            library.entries[1].id.as_str(),
            library.entries[2].id.as_str(),
            library.entries[3].id.as_str(),
            library.entries[4].kind(),
        ),
        ("ghz", "qft-4", "bell", "grover-search", CircuitKind::My)
    );
}

#[test]
fn delete_locked_entry_is_ignored() {
    let mut library = CircuitLibrary::from_entries(
        vec![
            user("open", "Open", EMPTY_CIRCUIT_JSON, false),
            user("locked", "Locked", EMPTY_CIRCUIT_JSON, true),
        ],
        "open".to_owned(),
    );

    library.delete_by_id("locked");

    assert!(library.entries.iter().any(|entry| entry.id == "locked"));
}

#[test]
fn rename_locked_entry_is_ignored() {
    let mut library = CircuitLibrary::from_entries(
        vec![user("locked", "Locked", EMPTY_CIRCUIT_JSON, true)],
        "locked".to_owned(),
    );

    library.rename("locked", "Renamed");

    assert_eq!(library.active().name.as_str(), "Locked");
}

#[test]
fn toggle_active_lock_flips_only_user_entries() {
    let mut library = CircuitLibrary::from_entries(
        vec![user("open", "Open", EMPTY_CIRCUIT_JSON, false)],
        "open".to_owned(),
    );

    library.toggle_active_lock();

    assert!(library.active_locked());
}

#[test]
fn toggle_active_lock_ignores_samples() {
    let mut library = CircuitLibrary::seed();

    let changed = library.toggle_active_lock();

    assert!(!changed);
}

#[test]
fn v1_migration_keeps_untouched_seed_count() {
    let seed = CircuitLibrary::seed();

    let migrated = CircuitLibrary::migrate_v1_entries(seed.entries, Some("bell".to_owned()));

    assert_eq!(migrated.entries.len(), 4);
}

#[test]
fn v1_migration_escapes_edited_seed_id() {
    let edited = CircuitEntry::user(
        "bell".to_owned(),
        "Bell state".to_owned(),
        EMPTY_CIRCUIT_JSON.to_owned(),
        0,
        false,
    );

    let migrated = CircuitLibrary::migrate_v1_entries(vec![edited], Some("bell".to_owned()));

    assert_eq!(migrated.active_id.as_str(), "bell-user-edit");
}

#[test]
fn v1_migration_marks_user_entry_unlocked() {
    let migrated = CircuitLibrary::migrate_v1_entries(
        vec![user("mine", "Mine", EMPTY_CIRCUIT_JSON, false)],
        Some("mine".to_owned()),
    );

    assert_eq!(
        migrated.active().origin,
        CircuitOrigin::User { locked: false }
    );
}

#[test]
fn startup_url_selects_canonical_sample() {
    let mut library = CircuitLibrary::seed();
    library.set_active_current_circuit(EMPTY_CIRCUIT_JSON.to_owned());

    library.resolve_startup_url_payload(r#"{"cols":[["H"],["•","X"]]}"#.to_owned());

    assert_eq!(library.active_id.as_str(), "bell");
}

#[test]
fn startup_url_preserves_locked_current_by_creating_fresh_entry() {
    let mut library = CircuitLibrary::seed();
    library.set_active_current_circuit(EMPTY_CIRCUIT_JSON.to_owned());
    library.toggle_active_lock();

    library.resolve_startup_url_payload(r#"{"cols":[["X"]]}"#.to_owned());

    assert_eq!(library.active_id.as_str(), "current-6");
}

#[test]
fn active_kind_reports_example_for_sample() {
    let library = CircuitLibrary::seed();

    assert_eq!(library.active_kind(), CircuitKind::Example);
}

#[test]
fn sample_origin_rejects_locked_field_at_compile_time() {
    let tests = trybuild::TestCases::new();

    tests.compile_fail("tests/compile_fail/sample_origin_locked.rs");
}
