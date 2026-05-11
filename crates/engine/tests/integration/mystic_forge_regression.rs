//! Regression: GitHub issue #297 — Mystic Forge ("You may cast artifact spells
//! and colorless spells from the top of your library.").
//!
//! User report (Discord): with Mystic Forge in play and a 3-CMC artifact on top
//! of library plus 3 mana available, the top-of-library card could not be
//! interacted with / cast.
//!
//! The parser correctly emits a `TopOfLibraryCastPermission` static whose
//! `affected` filter is an `Or` of `Typed[Artifact]` and `Typed[Card,
//! ColorCount EQ 0]`. The runtime helper `top_of_library_permission_source`
//! exists and is wired into `spell_objects_available_to_cast`. This regression
//! locks the end-to-end path so the user-reported scenario stays castable.
//!
//! CR 401.5 + CR 118.9 + CR 601.2a: the spell stays in `Zone::Library` until
//! `finalize_cast` performs the standard library→stack move; no exile step.

use std::path::Path;
use std::sync::OnceLock;

use engine::database::card_db::CardDatabase;
use engine::game::scenario::{GameScenario, P0};
use engine::game::scenario_db::GameScenarioDbExt;
use engine::types::actions::GameAction;
use engine::types::identifiers::ObjectId;
use engine::types::mana::{ManaType, ManaUnit};
use engine::types::phase::Phase;
use engine::types::player::PlayerId;
use engine::types::zones::Zone;

fn load_db() -> Option<&'static CardDatabase> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../client/public/card-data.json");
    if !path.exists() {
        return None;
    }
    static DB: OnceLock<CardDatabase> = OnceLock::new();
    Some(DB.get_or_init(|| CardDatabase::from_export(&path).expect("export should load")))
}

/// Move an object to the front of its owner's library so it is the "top card."
fn move_to_top_of_library(
    state: &mut engine::types::game_state::GameState,
    obj_id: ObjectId,
    owner: PlayerId,
) {
    let player = state.players.iter_mut().find(|p| p.id == owner).unwrap();
    player.library.retain(|id| *id != obj_id);
    player.library.push_front(obj_id);
    let obj = state.objects.get_mut(&obj_id).unwrap();
    obj.zone = Zone::Library;
}

fn add_mana(runner: &mut engine::game::scenario::GameRunner, mana: &[ManaType]) {
    let dummy = ObjectId(0);
    let pool = &mut runner
        .state_mut()
        .players
        .iter_mut()
        .find(|p| p.id == P0)
        .unwrap()
        .mana_pool;
    for m in mana {
        pool.add(ManaUnit::new(*m, dummy, false, vec![]));
    }
}

/// Issue #297: with Mystic Forge on the battlefield, a colorless artifact on
/// top of the controller's library must surface as castable through the
/// top-of-library permission path.
#[test]
fn mystic_forge_surfaces_colorless_artifact_on_library_top() {
    let Some(db) = load_db() else {
        return;
    };

    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    // Mystic Forge on the battlefield.
    let _forge_id = scenario.add_real_card(P0, "Mystic Forge", Zone::Battlefield, db);
    // Sol Ring is a colorless artifact — matches both arms of Mystic Forge's
    // `Or { Typed[Artifact], Typed[Card, ColorCount EQ 0] }` filter.
    let top_id = scenario.add_real_card(P0, "Sol Ring", Zone::Library, db);
    let mut runner = scenario.build();
    engine::game::rehydrate_game_from_card_db(runner.state_mut(), db);

    move_to_top_of_library(runner.state_mut(), top_id, P0);

    // Provide enough mana to satisfy Sol Ring's {1} cost so the full legal-
    // actions pipeline (filter + mana + timing) emits a CastSpell candidate.
    add_mana(&mut runner, &[ManaType::Colorless]);

    let available = engine::game::casting::spell_objects_available_to_cast(runner.state(), P0);
    assert!(
        available.contains(&top_id),
        "Mystic Forge must surface a colorless artifact on top of library; \
         available={:?}, top_id={:?}",
        available,
        top_id,
    );

    // CR 401.5: the card must remain in the library until cast finalizes.
    assert_eq!(
        runner.state().objects[&top_id].zone,
        Zone::Library,
        "Mystic Forge permission grant must NOT exile the top card"
    );

    // Full pipeline assertion: `legal_actions` (the source for `legalActions`
    // / `legalActionsByObject` over the WASM and WS bridges) must include a
    // CastSpell whose `object_id` is the library top. This is the failure
    // mode the user reported — without this, the frontend has no action to
    // surface no matter what the UI does.
    let legal = engine::ai_support::legal_actions(runner.state());
    let has_cast = legal.iter().any(|a| {
        matches!(
            a,
            GameAction::CastSpell { object_id, .. } if *object_id == top_id
        )
    });
    assert!(
        has_cast,
        "legal_actions must contain a CastSpell action for the library top \
         when Mystic Forge grants permission and mana is available; \
         legal_actions={:?}",
        legal,
    );
}

/// Negative regression: when no Mystic Forge (or similar) static is present,
/// the top of library must NOT surface as castable.
#[test]
fn no_static_means_top_of_library_not_castable() {
    let Some(db) = load_db() else {
        return;
    };

    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let top_id = scenario.add_real_card(P0, "Sol Ring", Zone::Library, db);
    let mut runner = scenario.build();
    engine::game::rehydrate_game_from_card_db(runner.state_mut(), db);

    move_to_top_of_library(runner.state_mut(), top_id, P0);

    let available = engine::game::casting::spell_objects_available_to_cast(runner.state(), P0);
    assert!(
        !available.contains(&top_id),
        "Without a TopOfLibraryCastPermission static, the top of library must NOT be castable"
    );
}

/// Sibling-cluster regression: Future Sight's broader "You may play lands and
/// cast spells from the top of your library" must also surface non-land cards
/// for casting.
#[test]
fn future_sight_surfaces_non_land_top_of_library() {
    let Some(db) = load_db() else {
        return;
    };

    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let _fs_id = scenario.add_real_card(P0, "Future Sight", Zone::Battlefield, db);
    // Any non-land card on top — Lightning Bolt is the canonical instant.
    let top_id = scenario.add_real_card(P0, "Lightning Bolt", Zone::Library, db);
    let mut runner = scenario.build();
    engine::game::rehydrate_game_from_card_db(runner.state_mut(), db);

    move_to_top_of_library(runner.state_mut(), top_id, P0);

    let available = engine::game::casting::spell_objects_available_to_cast(runner.state(), P0);
    assert!(
        available.contains(&top_id),
        "Future Sight must surface a non-land card on top of library; \
         available={:?}, top_id={:?}",
        available,
        top_id,
    );
}
