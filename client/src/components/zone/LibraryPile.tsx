import { useCallback, useMemo } from "react";

import type { GameAction, ObjectId } from "../../adapter/types.ts";
import { useCardImage } from "../../hooks/useCardImage.ts";
import { useGameDispatch } from "../../hooks/useGameDispatch.ts";
import { useCanActForWaitingState, usePlayerId } from "../../hooks/usePlayerId.ts";
import { CARD_BACK_URL } from "../../services/scryfall.ts";
import { useGameStore } from "../../stores/gameStore.ts";
import { useUiStore } from "../../stores/uiStore.ts";
import { collectObjectActions } from "../../viewmodel/cardActionChoice.ts";

interface LibraryPileProps {
  playerId: number;
  size?: { width: string; height: string };
}

function TopCard({ cardName }: { cardName: string }) {
  const { src } = useCardImage(cardName, { size: "normal" });

  if (!src) {
    return (
      <div
        className="h-full w-full rounded-lg bg-gray-700 border border-gray-600"
      />
    );
  }

  return (
    <img
      src={src}
      alt={cardName}
      className="h-full w-full rounded-lg object-cover"
      draggable={false}
    />
  );
}

/**
 * CR 401.5 + CR 118.9 + CR 601.2a: Filter `legalActionsByObject` entries for
 * the top-of-library card to the cast actions only. Mirrors `ZoneViewer`'s
 * exile-zone surfacing — Mystic Forge, Future Sight, Bolas's Citadel, Magus
 * of the Future, Realmwalker, etc. all surface a `CastSpell`-family action
 * for the library top through `spell_objects_available_to_cast`. The pile
 * displays whatever the engine reports; no per-mechanic permission inspection.
 */
function castActionsForObject(
  legalActionsByObject: Record<string, GameAction[]> | undefined,
  objectId: ObjectId,
): GameAction[] {
  return collectObjectActions(legalActionsByObject, objectId).filter((a) =>
    a.type === "CastSpell"
    || a.type === "CastSpellAsSneak"
    || a.type === "CastSpellAsWebSlinging"
    || a.type === "CastSpellAsMiracle"
    || a.type === "CastSpellAsMadness"
  );
}

export function LibraryPile({ playerId, size }: LibraryPileProps) {
  const myId = usePlayerId();
  const count = useGameStore(
    (s) => s.gameState?.players[playerId]?.library?.length ?? 0,
  );
  const canPeek = useGameStore(
    (s) =>
      playerId === myId &&
      (s.gameState?.players[playerId]?.can_look_at_top_of_library ?? false),
  );
  const topObjectId = useGameStore((s) => {
    const lib = s.gameState?.players[playerId]?.library;
    if (!lib || lib.length === 0) return null;
    // library[0] = top of library (engine convention from zones.rs)
    return lib[0];
  });
  const isRevealed = useGameStore((s) => {
    if (topObjectId == null) return false;
    return s.gameState?.revealed_cards?.includes(topObjectId) ?? false;
  });
  const topCardName = useGameStore((s) => {
    if (topObjectId == null) return null;
    const peek =
      playerId === myId &&
      (s.gameState?.players[playerId]?.can_look_at_top_of_library ?? false);
    const revealed = s.gameState?.revealed_cards?.includes(topObjectId) ?? false;
    if (!peek && !revealed) return null;
    return s.gameState?.objects[topObjectId]?.name ?? null;
  });

  const legalActionsByObject = useGameStore((s) => s.legalActionsByObject);
  const waitingFor = useGameStore((s) => s.waitingFor);
  const canActForWaitingState = useCanActForWaitingState();
  const setPendingAbilityChoice = useUiStore((s) => s.setPendingAbilityChoice);
  const dispatchAction = useGameDispatch();

  const isMyLibrary = playerId === myId;
  const hasPriority = waitingFor?.type === "Priority" && canActForWaitingState;

  // CR 401.5 + CR 118.9: cast-action surfacing is engine-authoritative —
  // the entry exists in `legalActionsByObject` only when the engine has
  // already validated the TopOfLibraryCastPermission filter, mana, and
  // timing. The frontend renders the reported actions, never computes them.
  const castActions = useMemo(() => {
    if (!isMyLibrary || !hasPriority || topObjectId == null) return [];
    return castActionsForObject(legalActionsByObject, topObjectId);
  }, [isMyLibrary, hasPriority, topObjectId, legalActionsByObject]);

  const canCast = castActions.length > 0;

  const handleCast = useCallback(() => {
    if (castActions.length === 0 || topObjectId == null) return;
    if (castActions.length === 1) {
      void dispatchAction(castActions[0]);
    } else {
      // Multiple cast options (e.g., cast normal + alt-cost) — defer to the
      // shared ability-choice modal so the player can pick.
      setPendingAbilityChoice({ objectId: topObjectId as ObjectId, actions: castActions });
    }
  }, [castActions, topObjectId, dispatchAction, setPendingAbilityChoice]);

  if (count === 0) return null;

  const stackDepth = Math.min(count - 1, 4);
  const isPeeking = (canPeek || isRevealed) && topCardName;
  const w = size?.width ?? "var(--card-w)";
  const h = size?.height ?? "var(--card-h)";

  return (
    <div
      className="relative"
      title={
        canCast
          ? `Cast ${topCardName ?? "top of library"} from top of library`
          : `Library (${count})`
      }
      data-library-pile={playerId}
      style={{ width: w, height: h }}
    >
      {/* Stack layers */}
      {Array.from({ length: stackDepth }).map((_, i) => (
        <div
          key={i}
          className="pointer-events-none absolute rounded-lg border border-gray-700 bg-gray-800"
          style={{
            width: w,
            height: h,
            bottom: (i + 1) * 3,
            left: (i + 1) * 1,
          }}
        />
      ))}

      {/* Top card */}
      <button
        type="button"
        onClick={canCast ? handleCast : undefined}
        disabled={!canCast}
        data-library-top-cast={canCast ? "true" : "false"}
        className={`relative block h-full w-full overflow-hidden rounded-lg border shadow-md ${
          canCast
            ? "border-amber-400 ring-2 ring-amber-400/70 shadow-[0_0_12px_3px_rgba(245,158,11,0.5)] cursor-pointer"
            : isRevealed
              ? "border-amber-500 cursor-default"
              : isPeeking
                ? "border-cyan-600 cursor-default"
                : "border-gray-600 cursor-default"
        }`}
      >
        {isPeeking ? (
          <TopCard cardName={topCardName} />
        ) : (
          <img
            src={CARD_BACK_URL}
            alt="Library"
            className="h-full w-full rounded-lg object-cover"
            draggable={false}
          />
        )}
      </button>

      {/* Count badge */}
      <div className="absolute -bottom-1 -right-1 z-10 flex h-5 w-5 items-center justify-center rounded-full bg-gray-900 text-[9px] font-bold text-gray-300 ring-1 ring-gray-600">
        {count}
      </div>
    </div>
  );
}
