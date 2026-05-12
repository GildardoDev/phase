use crate::types::ability::{EffectError, EffectKind, ResolvedAbility, TargetRef};
use crate::types::events::GameEvent;
use crate::types::game_state::GameState;

pub fn resolve(
    state: &mut GameState,
    ability: &ResolvedAbility,
    events: &mut Vec<GameEvent>,
) -> Result<(), EffectError> {
    let targets = crate::game::targeting::resolved_targets(
        ability,
        ability.effect.target_filter().unwrap(),
        state,
    );
    let Some(target_id) = targets.iter().find_map(|target| match target {
        TargetRef::Object(id) => Some(*id),
        TargetRef::Player(_) => None,
    }) else {
        events.push(GameEvent::EffectResolved {
            kind: EffectKind::PairWith,
            source_id: ability.source_id,
        });
        return Ok(());
    };

    // CR 702.95c-d: Resolution re-checks both would-be partners. If either is
    // no longer an unpaired creature on the battlefield under the soulbond
    // ability controller's control, neither object becomes paired.
    if crate::game::pairing::is_unpaired_creature_you_control(
        state,
        ability.source_id,
        ability.controller,
    ) && crate::game::pairing::is_unpaired_creature_you_control(
        state,
        target_id,
        ability.controller,
    ) {
        crate::game::pairing::pair_objects(state, ability.source_id, target_id);
    }

    events.push(GameEvent::EffectResolved {
        kind: EffectKind::PairWith,
        source_id: ability.source_id,
    });
    Ok(())
}
