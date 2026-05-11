import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { GameAction, GameObject, GameState } from "../../../adapter/types.ts";
import { useGameStore } from "../../../stores/gameStore.ts";
import { useUiStore } from "../../../stores/uiStore.ts";
import { LibraryPile } from "../LibraryPile.tsx";

vi.mock("../../../hooks/useCardImage", () => ({
  useCardImage: () => ({ src: null, isLoading: false }),
}));

const dispatchMock = vi.fn(async () => undefined);

vi.mock("../../../hooks/useGameDispatch.ts", () => ({
  useGameDispatch: () => dispatchMock,
}));

function makeObject(id: number, name: string): GameObject {
  return {
    id,
    card_id: id,
    owner: 0,
    controller: 0,
    zone: "Library",
    tapped: false,
    face_down: false,
    flipped: false,
    transformed: false,
    damage_marked: 0,
    dealt_deathtouch_damage: false,
    attached_to: null,
    attachments: [],
    counters: {},
    name,
    power: null,
    toughness: null,
    loyalty: null,
    card_types: { supertypes: [], core_types: ["Artifact"], subtypes: [] },
    mana_cost: { type: "Cost", shards: [], generic: 1 },
    keywords: [],
    abilities: [],
    trigger_definitions: [],
    replacement_definitions: [],
    static_definitions: [],
    color: [],
    base_power: null,
    base_toughness: null,
    base_keywords: [],
    base_color: [],
    timestamp: 1,
    entered_battlefield_turn: null,
  };
}

function setStore({
  topCardId = 42,
  canPeek,
  hasCastAction,
}: {
  topCardId?: number;
  canPeek: boolean;
  hasCastAction: boolean;
}) {
  const top = makeObject(topCardId, "Sol Ring");
  const gameState = {
    active_player: 0,
    objects: { [topCardId]: top },
    players: [
      {
        id: 0,
        life: 20,
        poison_counters: 0,
        mana_pool: { mana: [] },
        library: [topCardId],
        hand: [],
        graveyard: [],
        has_drawn_this_turn: false,
        lands_played_this_turn: 0,
        turns_taken: 0,
        can_look_at_top_of_library: canPeek,
      },
      {
        id: 1,
        life: 20,
        poison_counters: 0,
        mana_pool: { mana: [] },
        library: [],
        hand: [],
        graveyard: [],
        has_drawn_this_turn: false,
        lands_played_this_turn: 0,
        turns_taken: 0,
        can_look_at_top_of_library: false,
      },
    ],
    battlefield: [],
    exile: [],
    stack: [],
    combat: null,
    revealed_cards: [],
    waiting_for: { type: "Priority", data: { player: 0 } },
  } as unknown as GameState;

  const castAction: GameAction = {
    type: "CastSpell",
    data: { object_id: topCardId, card_id: topCardId, targets: [] },
  } as unknown as GameAction;

  useGameStore.setState({
    gameState,
    waitingFor: gameState.waiting_for,
    legalActions: hasCastAction ? [castAction] : [],
    legalActionsByObject: hasCastAction ? { [String(topCardId)]: [castAction] } : {},
    spellCosts: {},
    gameMode: "ai",
  });
  useUiStore.setState({
    pendingAbilityChoice: null,
  });
}

describe("LibraryPile cast surfacing", () => {
  beforeEach(() => {
    dispatchMock.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  /// Issue #297: Mystic Forge on the battlefield grants
  /// can_look_at_top_of_library + a CastSpell action keyed on the top object.
  /// The library pile must surface the cast button as clickable.
  it("dispatches the CastSpell action when the top card is castable", () => {
    setStore({ canPeek: true, hasCastAction: true });
    const { container } = render(<LibraryPile playerId={0} />);
    const button = container.querySelector(
      '[data-library-top-cast="true"]',
    ) as HTMLButtonElement;
    expect(button).not.toBeNull();
    expect(button.disabled).toBe(false);
    fireEvent.click(button);
    expect(dispatchMock).toHaveBeenCalledTimes(1);
    expect(dispatchMock).toHaveBeenCalledWith(
      expect.objectContaining({ type: "CastSpell" }),
    );
  });

  /// Negative: without a CastSpell legal action, the pile must not surface a
  /// cast button — clicking the pile is a no-op.
  it("does not dispatch when there is no cast action", () => {
    setStore({ canPeek: true, hasCastAction: false });
    const { container } = render(<LibraryPile playerId={0} />);
    const button = container.querySelector(
      '[data-library-top-cast="false"]',
    ) as HTMLButtonElement;
    expect(button).not.toBeNull();
    expect(button.disabled).toBe(true);
  });

  /// Without can_look_at_top_of_library, the card image stays hidden but the
  /// cast action surfacing remains engine-authoritative — when the engine
  /// reports a legal CastSpell, the button must still be enabled (the engine
  /// is the sole source of truth for castability).
  it("surfaces cast action even without peek when the engine reports one", () => {
    setStore({ canPeek: false, hasCastAction: true });
    const { container } = render(<LibraryPile playerId={0} />);
    const button = container.querySelector(
      '[data-library-top-cast="true"]',
    ) as HTMLButtonElement;
    expect(button).not.toBeNull();
    expect(button.disabled).toBe(false);
  });
});
