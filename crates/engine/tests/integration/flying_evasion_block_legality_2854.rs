//! Flying-evasion block legality (issue #2854).
//!
//! CR 509.1b + CR 702.9b + CR 702.17b: a creature with flying can't be blocked
//! except by creatures with flying and/or reach; disobeying that block
//! restriction (CR 509.1b) makes the blocker declaration illegal. Reach
//! (CR 702.17b) satisfies the flying restriction.
//!
//! Issue #2854 reported a flying token (Hornet Queen's Insect) being blockable
//! by a ground creature. The legality gate lives in
//! `combat::validate_blockers_for_player` (the authoritative declare-blockers
//! path reached by `engine_combat::handle_declare_blockers`). These tests drive
//! the *real* combat pipeline through `GameRunner.act(...)` rather than calling
//! the validator directly, so they exercise the same path production does.
//!
//! Two root causes are discriminated separately:
//!   - (a) the legality gate itself: the inline-keyword test fails if the flying
//!     restriction in `validate_blockers_for_player` is removed.
//!   - (b) the token keyword pipeline: the real-card test fails if a created
//!     Insect token stops surfacing `Flying` after layer evaluation, even though
//!     the gate is intact.
//!
//! The inline test builds creatures via `GameScenario` with inline keywords so it
//! runs in CI without `client/public/card-data.json`. The real-card test models
//! Hornet Queen's token-creating ETB from Oracle text (parsed at build time, no
//! card-data.json dependency), driving the token through the cast/ETB/layer
//! pipeline and asserting `GameObject::has_keyword(&Keyword::Flying)` post-layers.

use engine::game::game_object::GameObject;
use engine::game::scenario::{GameRunner, GameScenario, P0, P1};
use engine::types::actions::GameAction;
use engine::types::card_type::CoreType;
use engine::types::game_state::WaitingFor;
use engine::types::identifiers::ObjectId;
use engine::types::keywords::Keyword;
use engine::types::phase::Phase;

use super::rules::AttackTarget;

/// Drive a single P0 attacker against P1 from PreCombatMain up to the
/// declare-blockers window, then submit `blocks` and return the raw `Result`
/// from the declare-blockers action — so the caller can assert legality.
///
/// CR 508.2 + CR 509.1 + CR 117.1c: pass through the post-attack priority window
/// to reach the declare-blockers step before submitting blocks.
fn declare_block_result(
    runner: &mut GameRunner,
    attacker: ObjectId,
    blocks: Vec<(ObjectId, ObjectId)>,
) -> Result<(), String> {
    runner.pass_both_players();
    runner
        .act(GameAction::DeclareAttackers {
            attacks: vec![(attacker, AttackTarget::Player(P1))],
            bands: vec![],
        })
        .expect("DeclareAttackers should succeed");
    if matches!(runner.state().waiting_for, WaitingFor::Priority { .. }) {
        runner.pass_both_players();
    }
    assert!(
        matches!(
            runner.state().waiting_for,
            WaitingFor::DeclareBlockers { .. }
        ),
        "expected a DeclareBlockers window, got {:?}",
        runner.state().waiting_for
    );
    runner
        .act(GameAction::DeclareBlockers {
            assignments: blocks,
        })
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}

/// Build a fresh scenario with a flying 1/1 attacker (P0) and the three blocker
/// archetypes (P1): a ground 2/2 (no flying, no reach), a Reach 2/2, and a
/// Flying 2/2. Models the Insect token's evaluated state (Flying + Deathtouch).
/// Returns `(runner, attacker, ground, reach, flyer)`.
fn flying_attacker_three_blockers() -> (GameRunner, ObjectId, ObjectId, ObjectId, ObjectId) {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);

    let attacker = scenario
        .add_creature(P0, "Flying Insect", 1, 1)
        .with_keyword(Keyword::Flying)
        .with_keyword(Keyword::Deathtouch)
        .id();
    let ground = scenario.add_creature(P1, "Ground Blocker", 2, 2).id();
    let reach = scenario
        .add_creature(P1, "Reach Blocker", 2, 2)
        .with_keyword(Keyword::Reach)
        .id();
    let flyer = scenario
        .add_creature(P1, "Flying Blocker", 2, 2)
        .with_keyword(Keyword::Flying)
        .id();

    (scenario.build(), attacker, ground, reach, flyer)
}

/// CR 702.9b: a ground creature (no flying, no reach) declaring a block on a
/// flying attacker is illegal — the declare-blockers action is rejected
/// (CR 509.1b). This is the core defect #2854 reported.
///
/// Discriminating assertion: this `expect_err` flips to a panic if the flying
/// restriction in `validate_blockers_for_player` is removed (root cause a).
#[test]
fn ground_creature_cannot_block_flyer() {
    let (mut runner, attacker, ground, _reach, _flyer) = flying_attacker_three_blockers();
    let result = declare_block_result(&mut runner, attacker, vec![(ground, attacker)]);
    result.expect_err("CR 702.9b: a ground blocker may not block a flying attacker");
}

/// CR 702.17b: reach satisfies the flying block restriction — a reach blocker
/// may legally block a flying attacker. Control proving the gate keys on the
/// flying/reach keywords, not on a blanket rejection of all blocks.
#[test]
fn reach_creature_can_block_flyer() {
    let (mut runner, attacker, _ground, reach, _flyer) = flying_attacker_three_blockers();
    let result = declare_block_result(&mut runner, attacker, vec![(reach, attacker)]);
    result.expect("CR 702.17b: a reach blocker may legally block a flying attacker");
}

/// CR 702.9b: a flying blocker may legally block a flying attacker. Second
/// control confirming the legal arm of the same gate.
#[test]
fn flying_creature_can_block_flyer() {
    let (mut runner, attacker, _ground, _reach, flyer) = flying_attacker_three_blockers();
    let result = declare_block_result(&mut runner, attacker, vec![(flyer, attacker)]);
    result.expect("CR 702.9b: a flying blocker may legally block a flying attacker");
}

const HORNET_QUEEN_ETB: &str = "When Hornet Queen enters, create four 1/1 green \
     Insect creature tokens with flying and deathtouch.";

/// Real-card analogue: drive Hornet Queen's token-creating ETB through the
/// cast/ETB/layer pipeline, locate a created Insect token, and assert it
/// surfaces `Flying` (and `Deathtouch`) *after* layer evaluation (root cause b).
///
/// Modeling the ETB from Oracle text (rather than loading the printed card)
/// keeps the test card-data-independent while still exercising the real token
/// creation → `base_keywords` write → layer-reset-preserves-keywords path. The
/// discriminating assertion is `has_keyword(&Keyword::Flying)` on the token
/// (flips if the token loses flying through layers). Block legality for a flying
/// attacker is covered by `ground_creature_cannot_block_flyer` above; this test
/// deliberately does not re-drive combat, because the freshly-created token has
/// summoning sickness (CR 302.6) and cannot attack the turn it enters.
#[test]
fn hornet_queen_insect_token_enters_with_flying_and_deathtouch() {
    let mut scenario = GameScenario::new();
    scenario.at_phase(Phase::PreCombatMain);
    let queen = scenario
        .add_creature_to_hand_from_oracle(P0, "Hornet Queen", 2, 2, HORNET_QUEEN_ETB)
        .id();

    let mut runner = scenario.build();

    // Cast Hornet Queen; the ETB resolves and creates the Insect tokens. The
    // runner's live state carries the resolution result after `resolve()`.
    let _ = runner.cast(queen).resolve();

    // Locate an Insect token: a token creature controlled by P0 that is not the
    // Queen herself (CR 111.2: the player creating a token is its owner).
    let token = runner
        .state()
        .objects
        .values()
        .find(|o: &&GameObject| {
            o.is_token
                && o.controller == P0
                && o.id != queen
                && o.card_types.core_types.contains(&CoreType::Creature)
        })
        .expect("Hornet Queen's ETB should create an Insect token under P0");

    // Root cause (b): the token must still have flying AND deathtouch AFTER layer
    // evaluation — `base_keywords` written at creation must survive the
    // battlefield-entry layer reset.
    assert!(
        token.has_keyword(&Keyword::Flying),
        "CR 702.9b: the created Insect token must retain flying through layers"
    );
    assert!(
        token.has_keyword(&Keyword::Deathtouch),
        "the created Insect token must retain deathtouch through layers"
    );
}
