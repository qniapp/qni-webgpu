use super::{CircuitEntry, CircuitLibrary, EMPTY_CIRCUIT_JSON};

#[test]
fn seed_contains_three_named_samples() {
    let library = CircuitLibrary::seed();

    assert_eq!(
        (
            library.entries.len(),
            library.active_id.as_str(),
            library.entries[0].name.as_str(),
            library.entries[1].name.as_str(),
            library.entries[2].name.as_str(),
        ),
        (3, "bell", "Bell state", "GHZ state", "QFT 4-qubit")
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
            CircuitEntry {
                id: "current".to_owned(),
                name: "Untitled".to_owned(),
                circuit_json: EMPTY_CIRCUIT_JSON.to_owned(),
                updated_at: 0,
            },
            CircuitEntry {
                id: "circuit-8".to_owned(),
                name: "Untitled".to_owned(),
                circuit_json: EMPTY_CIRCUIT_JSON.to_owned(),
                updated_at: 0,
            },
            CircuitEntry {
                id: "ckt_saved".to_owned(),
                name: "Untitled".to_owned(),
                circuit_json: EMPTY_CIRCUIT_JSON.to_owned(),
                updated_at: 0,
            },
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
fn update_and_set_active_keep_canonical_json() {
    let mut library = CircuitLibrary::seed();

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
fn duplicate_move_and_delete_preserve_active_invariant() {
    let mut library = CircuitLibrary::seed();

    let Some(duplicated) = library.duplicate(1).cloned() else {
        panic!("duplicate entry should be created");
    };
    let after_duplicate = (
        duplicated.name.clone(),
        duplicated.updated_at != 0,
        library.active_id.clone(),
        library.entries[2].id.clone(),
    );

    library.move_up(2);
    let after_move_up_id = library.entries[1].id.clone();
    library.move_down(1);
    let after_move_down_id = library.entries[2].id.clone();

    library.delete(2);
    assert_eq!(
        (
            after_duplicate.0.as_str(),
            after_duplicate.1,
            after_duplicate.2.as_str(),
            after_duplicate.3.as_str(),
            after_move_up_id.as_str(),
            after_move_down_id.as_str(),
            library.active_id.as_str(),
            library.entries.len(),
        ),
        (
            "GHZ state (copy)",
            true,
            duplicated.id.as_str(),
            duplicated.id.as_str(),
            duplicated.id.as_str(),
            duplicated.id.as_str(),
            "bell",
            3,
        )
    );
}

#[test]
fn duplicate_active_inserts_after_active_and_numbers_copy_names() {
    let mut library = CircuitLibrary::seed();
    library.set_active("bell".to_owned());
    if let Some(entry) = library.entries.iter_mut().find(|entry| entry.id == "bell") {
        entry.updated_at = 0;
    }

    let first_id = library.duplicate_active();
    let first_snapshot = (
        library.entries[1].id.clone(),
        library.active_id.clone(),
        library.entries[1].name.clone(),
        library.entries[1].circuit_json.clone(),
        library.entries[0].circuit_json.clone(),
        library.active().updated_at != 0,
    );

    let second_id = library.duplicate_active();
    let second_snapshot = (
        library.entries[2].id.clone(),
        library.entries[2].name.clone(),
    );

    let third_id = library.duplicate_active();
    assert_eq!(
        (
            first_snapshot.0.as_str(),
            first_snapshot.1.as_str(),
            first_snapshot.2.as_str(),
            first_snapshot.3.as_str(),
            first_snapshot.4.as_str(),
            first_snapshot.5,
            second_snapshot.0.as_str(),
            second_snapshot.1.as_str(),
            library.entries[3].id.as_str(),
            library.entries[3].name.as_str(),
        ),
        (
            first_id.as_str(),
            first_id.as_str(),
            "Bell state (copy)",
            first_snapshot.4.as_str(),
            first_snapshot.4.as_str(),
            true,
            second_id.as_str(),
            "Bell state (copy 2)",
            third_id.as_str(),
            "Bell state (copy 3)",
        )
    );
}

#[test]
fn duplicate_active_skips_existing_copy_name_collisions() {
    let mut library = CircuitLibrary::seed();
    library.entries[1].name = "Bell state (copy)".to_owned();
    library.entries[2].name = "Bell state (copy 2)".to_owned();
    library.set_active("bell".to_owned());

    let id = library.duplicate_active();

    assert_eq!(
        (library.active_id.as_str(), library.entries[1].name.as_str()),
        (id.as_str(), "Bell state (copy 3)")
    );
}

#[test]
fn reorder_moves_by_insertion_index_and_preserves_active_id() {
    let mut library = CircuitLibrary::seed();
    library.set_active("ghz".to_owned());

    library.reorder(0, 3);

    assert_eq!(
        (
            library
                .entries
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            library.active_id.as_str(),
        ),
        (vec!["ghz", "qft-4", "bell"], "ghz")
    );
}

#[test]
fn reorder_ignores_no_ops_and_out_of_bounds_source() {
    let mut library = CircuitLibrary::seed();
    let original = library.clone();

    library.reorder(2, 2);
    let after_same_index = library.clone();

    library.reorder(2, 3);
    let after_endpoint_noop = library.clone();

    library.reorder(99, 0);
    assert_eq!(
        (after_same_index, after_endpoint_noop, library),
        (original.clone(), original.clone(), original)
    );
}

#[test]
fn swap_adjacent_swaps_without_touching_active_timestamp() {
    let mut library = CircuitLibrary::seed();
    library.set_active("ghz".to_owned());
    if let Some(entry) = library.entries.iter_mut().find(|entry| entry.id == "ghz") {
        entry.updated_at = 0;
    }

    library.swap_adjacent(1, 2);

    assert_eq!(
        (
            library
                .entries
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            library.active_id.as_str(),
            library.active().updated_at,
        ),
        (vec!["bell", "qft-4", "ghz"], "ghz", 0)
    );
}
